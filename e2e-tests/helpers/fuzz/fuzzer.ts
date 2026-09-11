/**
 * Random-move fuzzing of the real app: a `Fuzzer` holds the agents driven to
 * where moves expect them and the model of what each should be showing, and
 * runs sequences of moves against them — drawn at random from the pools a
 * spec passes, to search for a failing one and shrink it to the smallest that
 * fails, or once to soak the app in ordinary use — or replayed from a report.
 * Every move checks whoever the model says learnt something, so the property
 * is the sequence itself. The property-testing machinery underneath is
 * entirely its own business.
 */
import fc from 'fast-check';

import { leaveWifi, wifiDevice } from '../../setup/host-wifi';
import type { Agent } from '../../setup/setup-agents';
import type { WifiNetwork } from '../../setup/test-env';
import { navigateToAddContact } from '../flows/exchange-contacts';
import { createGroup } from '../flows/exchange-contacts-and-create-group';
import {
	type Real,
	type StressAgent,
	addContact,
	ensureHome,
	goHome,
	log,
	newReal,
	openChatPage,
	parkHub,
} from './agents';
import { type ExpectedModel, newModel } from './model';
import type { Move, Moves } from './moves/move';

export type { Move, Moves } from './moves/move';

type Sequence = Iterable<fc.AsyncCommand<ExpectedModel, Real>>;

function oneOf(moves: Moves): fc.Arbitrary<Move> {
	if (moves.length === 0) throw new Error('no moves to draw from');
	return fc.oneof(...moves);
}

/** A fuzz test runs as long as its moves take: `prepare` lifts the mocha
 *  timeout of every test in its suite to this. It has to happen before a
 *  test starts, since wdio arms its per-test abort timer from that timeout
 *  at that moment. `search` bounds itself through fast-check instead. */
const FUZZ_TEST_TIMEOUT_MS = 24 * 60 * 60 * 1_000;

/** What a move may take: it drives a real UI flow and its checks wait out
 *  generous sync timeouts. */
const MOVE_BUDGET_MS = 30_000;

/** What a sequence takes besides its moves: the network reset before it and
 *  the teardown and convergence check after it. */
const SEQUENCE_BUDGET_MS = 300_000;

export interface SearchOptions {
	moves: Moves;
	/** How many sequences to try before giving up on finding a failure. */
	attempts: number;
	/** The most moves a sequence may have. */
	length: number;
	/** Random by default; the run logs it, and a failure reports it. */
	seed?: number;
}

export interface SoakOptions {
	moves: Moves;
	/** Moves in the sequence. */
	length: number;
	seed?: number;
}

interface RunParams {
	numRuns: number;
	seed?: number;
	endOnFailure?: boolean;
	interruptAfterTimeLimit?: number;
}

export class Fuzzer {
	private constructor(
		private readonly model: ExpectedModel,
		private readonly real: Real,
	) {}

	/**
	 * A fuzzer over `agents`, driven to where moves expect them: preview
	 * features on, each one's contact link collected, and — given networks
	 * to walk phones and hubs through, and a Wi-Fi card on the host for the
	 * hubs — a members-less group chat each, to read the connection chip in.
	 * Every agent is left on its home page. Preparation is not repeatable,
	 * so a spec prepares once, from its `before` hook — whose context `ctx`
	 * is, so that the suite's tests can be freed of their timeout before any
	 * of them starts — and runs as often as it likes.
	 */
	static async prepare(
		ctx: Mocha.Context,
		init: {
			agents: { agent: Agent; name: string }[];
			networks?: WifiNetwork[];
		},
	): Promise<Fuzzer> {
		const suite = ctx.test?.parent;
		if (
			!ctx.test?.title.startsWith('"before all" hook') ||
			suite === undefined
		) {
			throw new Error(
				"Fuzzer.prepare must be called from the suite's before()",
			);
		}
		suite.eachTest(test => test.timeout(FUZZ_TEST_TIMEOUT_MS));
		const networked = init.networks !== undefined && init.networks.length > 0;
		const real = newReal({
			...init,
			hubsDevice: networked ? wifiDevice() : null,
		});
		if (networked && real.hubsDevice === null) {
			throw new Error('networks are configured but the host has no Wi-Fi card');
		}
		const model = newModel(real);
		await prepareAgents(model, real);
		return new Fuzzer(model, real);
	}

	/**
	 * Try up to `attempts` random sequences of at most `length` moves. The
	 * first that fails is shrunk to the smallest failing sequence — dropping
	 * moves, then lowering their arguments — for as long again as the search
	 * was given, and reported with the seed as `move` builders ready to paste
	 * into a `replay`.
	 */
	search(opts: SearchOptions): Promise<void> {
		const budget = 2 * opts.attempts * sequenceBudget(opts.length);
		return this.run(
			fc.commands([oneOf(opts.moves)], {
				maxCommands: opts.length,
				size: 'max',
			}),
			{
				numRuns: opts.attempts,
				seed: opts.seed,
				interruptAfterTimeLimit: budget,
			},
		);
	}

	/** One long random sequence: the app under ordinary use for a while. A
	 *  failure is reported as is, without shrinking. */
	soak(opts: SoakOptions): Promise<void> {
		return this.run(
			fc.array(oneOf(opts.moves), {
				minLength: opts.length,
				maxLength: opts.length,
			}),
			{ numRuns: 1, seed: opts.seed, endOnFailure: true },
		);
	}

	/** Run a reported sequence exactly as the search that found it did. */
	replay(moves: Move[]): Promise<void> {
		return this.run(fc.constant(moves), { numRuns: 1, endOnFailure: true });
	}

	/**
	 * The property "any sequence drawn from `sequences` keeps the real system
	 * matching the model", every move checking it: the network side is reset
	 * before each sequence and whatever it raised is torn down after the run.
	 * A failure throws the report plus the sequence — shrunk, where
	 * `params` allow it — as `move` builders.
	 */
	private async run(
		sequences: fc.Arbitrary<Sequence>,
		params: RunParams,
	): Promise<void> {
		const { model, real } = this;
		const seed = params.seed ?? Math.floor(Math.random() * 2 ** 31);
		log(`seed ${seed}`);
		let out: fc.RunDetails<[Sequence]>;
		try {
			out = await fc.check(
				fc.asyncProperty(sequences, moves =>
					fc.asyncModelRun(async () => {
						await resetNetworks(model, real);
						return { model, real };
					}, moves),
				),
				// Unbiased: the bias draws early runs' sequences short and their
				// arguments small, and a run of one is all "early".
				{ ...params, seed, unbiased: true, includeErrorInReport: true },
			);
		} finally {
			// A sequence that failed mid-way skipped its own teardown.
			await teardown(real);
		}
		if (out.failed) {
			const moves = out.counterexample?.[0];
			throw new Error(
				`${fc.defaultReportMessage(out)}\n\nSeed: ${seed}\nReproduction: ` +
					(moves === undefined ? 'none reported' : asReproduction(moves)),
			);
		}
	}
}

function sequenceBudget(length: number): number {
	return length * MOVE_BUDGET_MS + SEQUENCE_BUDGET_MS;
}

/** Put a phone back on its usual network however a run left it: on the
 *  air, with every test network forgotten so the supplicant cannot pick one
 *  again. Throws if no saved network is in range. */
async function restorePhone(
	sa: StressAgent,
	networks: WifiNetwork[],
): Promise<void> {
	await sa.agent.enableWifi();
	for (const network of networks) await sa.agent.forgetWifi(network.ssid);
}

/** The host's card and every phone back on their usual networks. */
async function restoreNetworks(real: Real): Promise<void> {
	for (const network of real.networks) leaveWifi(network.ssid);
	for (const sa of real.agents) await restorePhone(sa, real.networks);
}

async function prepareAgents(model: ExpectedModel, real: Real): Promise<void> {
	if (model.hasNetworks()) await restoreNetworks(real);
	for (const sa of real.agents) {
		await sa.agent.enablePreviewFeatures();
		await sa.agent.homePage.ready();
		await navigateToAddContact(sa.agent);
		sa.link = await sa.agent.addContactPage.getAddContactLink();
		await sa.agent.addContactPage.back.click();
		await sa.agent.newMessagePage.back.click();
		await sa.agent.homePage.ready();
		if (!model.hasNetworks()) continue;
		const chatName = model.nextGroupName();
		await createGroup(sa.agent, chatName, []);
		await ensureHome(sa);
		model.addGroup(sa.name, [], chatName);
	}
	if (!model.hasNetworks()) return;
	// The peer checks probe every direct chat, so every pair are contacts
	// before the first sequence — established, profiles included, while the
	// phones still share their usual LAN.
	for (const sa of real.agents) {
		for (const peer of real.agents) {
			if (peer === sa) continue;
			await addContact(sa, peer);
			model.recordAdded(sa.name, peer.name);
		}
	}
	for (const sa of real.agents) {
		for (const peer of real.agents) {
			if (peer === sa) continue;
			await openChatPage(sa, model.directChat(sa.name, peer.name), model);
			await sa.agent.directChatPage.waitForPeerProfile();
			await goHome(sa, sa.agent.directChatPage);
		}
	}
	model.propagateShared();
}

/** Park every hub and take the host's card off the test networks. */
async function parkHubs(real: Real): Promise<void> {
	for (const hub of real.hubs) await parkHub(hub);
	if (real.hubsNetwork !== null) leaveWifi(real.hubsNetwork);
	real.hubsNetwork = null;
}

/**
 * Put the network side back to its starting state — every hub parked and on
 * no LAN, every phone foregrounded, at home and off the air — so that every
 * sequence, drawn or shrunk, begins from the same place. Chats and messages
 * are left alone: they carry over as they do on the devices, and so does what
 * the model says each agent knows.
 */
async function resetNetworks(model: ExpectedModel, real: Real): Promise<void> {
	if (!model.hasNetworks()) return;
	await parkHubs(real);
	for (const hub of model.hubs) {
		hub.network = null;
		hub.running = false;
	}
	for (const sa of real.agents) {
		await sa.agent.startApp();
		model.foreground(sa.name);
		await ensureHome(sa);
		await sa.agent.disableWifi();
		model.agentLeave(sa.name);
	}
}

/** Leave nothing of a run behind: hubs down, the host's card and the phones
 *  back on their usual networks. The test networks stay on the air, so the
 *  phones forget them or the supplicant may pick one again. */
async function teardown(real: Real): Promise<void> {
	await parkHubs(real);
	if (real.networks.length === 0) return;
	for (const network of real.networks) leaveWifi(network.ssid);
	for (const sa of real.agents) {
		try {
			await restorePhone(sa, real.networks);
		} catch {
			/* no saved network in range; nothing to restore */
		}
	}
}

/** A reported sequence, as the `move` builders that replay it. */
function asReproduction(moves: Iterable<{ toString(): string }>): string {
	return `[${[...moves].map(m => `move.${String(m)}`).join(', ')}]`;
}
