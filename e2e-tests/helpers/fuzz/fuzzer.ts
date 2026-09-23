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

import { assertInRange, leaveWifi, wifiDevice } from '../../setup/host-wifi';
import {
	healMailboxLink,
	mailboxDegradable,
	mailboxServing,
	mailboxWakesPhones,
} from '../../setup/mailbox-control';
import type { Agent } from '../../setup/setup-agents';
import type { WifiNetwork } from '../../setup/test-env';
import type { RenderedMessage } from '../components/messages';
import { contactLinkOf } from '../flows/exchange-contacts';
import { createGroup } from '../flows/exchange-contacts-and-create-group';
import {
	type Real,
	type StressAgent,
	ensureHome,
	labNetworks,
	log,
	newReal,
	openChatByTitle,
	parkHub,
	readNotificationTexts,
} from './agents';
import { expectNotifications } from './checks';
import {
	type ExpectedChat,
	type ExpectedModel,
	type MessageKind,
	newModel,
} from './model';
import { type Move, type Moves, steps } from './moves/move';

export type { Move, Moves } from './moves/move';

type Sequence = Iterable<fc.AsyncCommand<ExpectedModel, Real>>;

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
	 * A fuzzer over `agents` — the ones a spec has already set up and given
	 * profiles, by name, as `createProfiles` takes them — driven to where
	 * moves expect them: preview features on, each
	 * one's contact link collected, and — given networks to walk phones and
	 * hubs through, and a Wi-Fi card on the host for the hubs, or the `cloud`
	 * mailbox's link to degrade — a members-less group chat each, to read the
	 * connection chip in. It makes no contacts and seeds nothing: it reads
	 * whatever the agents already have — who they are contacts of, the groups
	 * they are in, the messages in each chat — and starts the model from
	 * that, so a spec can exchange contacts itself beforehand, or hand over
	 * agents another run left behind, and a run's own `addContact` moves are
	 * checked like any other move. The network the phones are on to begin with is
	 * the run's home network: phones may walk onto it, and the hubs are on it
	 * whenever the card is. Preparation is not repeatable, so a spec prepares
	 * once, from its `before` hook — whose context `ctx` is, so that the
	 * suite's tests can be freed of their timeout before any of them starts —
	 * and runs as often as it likes.
	 */
	static async prepare(
		ctx: Mocha.Context,
		init: {
			/** The agents the run drives, by the profile name each goes by:
			 *  unique, and none a substring of another, since a name is how a
			 *  chat row, a notification and a move report name its agent. */
			agents: Record<string, Agent>;
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
			// A spec that took the mailbox down before preparing gets a model
			// whose cloud is already unreachable, without having to say so.
			cloudUsable: await mailboxServing(),
			push: await mailboxWakesPhones(),
		});
		if (networked) {
			if (real.hubsDevice === null) {
				throw new Error(
					'networks are configured but the host has no Wi-Fi card',
				);
			}
			await restoreNetworks(real);
			// However the run ends: a search that fails partway leaves every
			// phone wherever `resetAgent` last put it, which is off Wi-Fi, and
			// the host's card on a lab network. Nothing else puts them back,
			// so every later run on these devices starts off the air.
			suite.afterAll('restore networks', () => restoreNetworks(real));
			assertInRange(
				real.hubsDevice,
				labNetworks(real).map(n => n.ssid),
			);
		}
		for (const sa of real.agents) {
			sa.notificationTexts = await readNotificationTexts(sa);
		}
		const model = newModel(real);
		await prepareAgents(model, real);
		await recordExistingState(model, real);
		// The devices start where the model says they do — nothing showing,
		// the clear above having taken. Asserting it here makes a clear that
		// did not work say so, instead of surfacing as the first move being
		// blamed for a notification an earlier run left behind.
		await expectNotifications(model, real);
		return new Fuzzer(model, real);
	}

	/**
	 * Try up to `attempts` random sequences of `length` moves, each step
	 * resolving to a move that can be made when it is reached. The first
	 * sequence that fails is shrunk to the smallest failing one — dropping
	 * moves, then lowering their arguments — for as long again as the search
	 * was given, and reported with the seed as `move` builders ready to paste
	 * into a `replay`.
	 */
	search(opts: SearchOptions): Promise<void> {
		return this.run(
			fc.commands([steps(opts.moves)], {
				maxCommands: opts.length,
				size: 'max',
			}),
			{
				numRuns: opts.attempts,
				seed: opts.seed,
			},
		);
	}

	/** One long random sequence: the app under ordinary use for a while. A
	 *  failure is reported as is, without shrinking. */
	soak(opts: SoakOptions): Promise<void> {
		return this.run(
			fc.array(steps(opts.moves), {
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
	 * matching the model", every move checking it. A sequence starts with every
	 * app on screen, carrying whatever else the last one left — the model holds
	 * that, and every move's `check` gates on it — and whatever the run raised
	 * is torn down after it.
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
		let sequence = 0;
		try {
			out = await fc.check(
				fc.asyncProperty(sequences, moves => {
					sequence++;
					log(`sequence ${sequence}: ${[...moves].length} moves drawn`);
					return fc.asyncModelRun(async () => {
						await resetSequence(model, real);
						return { model, real };
					}, moves);
				}),
				// Unbiased: the bias draws early runs' sequences short and their
				// arguments small, and a run of one is all "early".
				{ ...params, seed, unbiased: true, includeErrorInReport: true },
			);
		} finally {
			// A sequence that failed mid-way skipped its own teardown.
			await teardown(real);
		}
		log(
			`${out.numRuns} sequences run, ${out.numSkips} skipped, interrupted=${String(out.interrupted)}, failed=${String(out.failed)}`,
		);
		if (out.failed) {
			const moves = out.counterexample?.[0];
			throw new Error(
				`${fc.defaultReportMessage(out)}\n\nSeed: ${seed}\nReproduction: ` +
					(moves === undefined ? 'none reported' : asReproduction(moves)),
			);
		}
	}
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

/** The network every phone is on to begin with is the run's home network:
 *  the listed entry of that name, or that name alone when it is not listed.
 *  Read before anything is left or forgotten, so the run cannot take the
 *  network everyone sits on for a lab one. */
async function inferHomeNetwork(real: Real): Promise<void> {
	const infos = await Promise.all(real.agents.map(sa => sa.agent.wifiInfo()));
	const ssids = new Set(infos.map(info => info.ssid));
	if (ssids.size !== 1) {
		throw new Error(
			`the phones are on different networks to begin with (${[...ssids].join(', ')}); put them on the host's network`,
		);
	}
	const [ssid] = ssids;
	if (ssid === '') {
		console.log('[wifi] the phones are on no network; no home network');
		return;
	}
	const listed = real.networks.find(n => n.ssid === ssid);
	if (listed !== undefined) listed.home = true;
	else real.networks.push({ ssid, passphrase: '', home: true });
	console.log(`[wifi] home network: ${ssid}`);
}

/** The host's card and every phone back on their usual networks. */
async function restoreNetworks(real: Real): Promise<void> {
	// Per phone and independent, so every phone does it at once; only the
	// host's own card has to be taken off the lab networks in turn.
	await Promise.all(real.agents.map(sa => sa.agent.enableWifi()));
	await inferHomeNetwork(real);
	for (const network of labNetworks(real)) await leaveWifi(network.ssid);
	await Promise.all(real.agents.map(sa => restorePhone(sa, labNetworks(real))));
}

/** Drive every agent to where moves expect it. Each drives its own session
 *  and reads nothing of another's, so they are prepared at once: with a
 *  handful of agents this is the difference between one wait and n of them.
 *  The group names are drawn before, and recorded after, so the shared model
 *  is only ever touched from here. */
async function prepareAgents(model: ExpectedModel, real: Real): Promise<void> {
	const chipChats = new Map<StressAgent, string>();
	if (model.watchesChip()) {
		for (const sa of real.agents) chipChats.set(sa, model.nextGroupName());
	}
	await Promise.all(
		real.agents.map(sa => prepareAgent(model, sa, chipChats.get(sa) ?? null)),
	);
	for (const [sa, chatName] of chipChats) {
		model.setChipChat(sa.name, model.addGroup(sa.name, [], chatName));
		model.wentHome(sa.name);
	}
}

/** What one agent's screen says it already has: the chats in its list, and
 *  what is in each of them. */
interface FoundState {
	sa: StressAgent;
	titles: string[];
	chats: Map<string, RenderedMessage[]>;
}

/**
 * Build the model from what the agents already have, so a spec can set them
 * up however it likes — exchange contacts, seed chats, or hand over agents a
 * previous run left behind — and the run carries on from there.
 *
 * Chats are read from the lists: a row titled with another agent's name is
 * that pair's direct chat, and any other row is a group whose members are
 * exactly the agents whose lists hold it.
 */
async function recordExistingState(
	model: ExpectedModel,
	real: Real,
): Promise<void> {
	const found = await Promise.all(real.agents.map(readState));
	const names = new Set(real.agents.map(sa => sa.name));
	for (const { sa, titles } of found) {
		for (const title of titles) {
			if (names.has(title)) model.recordExistingContacts(sa.name, title);
		}
	}
	for (const title of new Set(found.flatMap(f => f.titles))) {
		if (names.has(title)) continue;
		const members = found.filter(f => f.titles.includes(title)).map(f => f.sa);
		model.recordExistingGroup(
			title,
			members.map(member => member.name),
		);
	}
	recordExistingMessages(model, found);
	if (found.some(f => f.titles.length > 0)) {
		log(`started from ${describeFound(found)}`);
	}
}

/** Read one agent's chat list and every chat in it. */
async function readState(sa: StressAgent): Promise<FoundState> {
	await sa.agent.homePage.ready();
	const titles = await sa.agent.homePage.chatTitles();
	const chats = new Map<string, RenderedMessage[]>();
	for (const title of titles) {
		const page = await openChatByTitle(sa, title);
		chats.set(title, await page.messages.renderedMessages());
		await page.back.click();
		await sa.agent.homePage.ready();
	}
	return { sa, titles, chats };
}

/** Put every message the agents were already showing into the model, each
 *  known by exactly the agents whose screen had it. */
function recordExistingMessages(
	model: ExpectedModel,
	found: FoundState[],
): void {
	for (const { sa, chats } of found) {
		for (const [title, rendered] of chats) {
			const chat = model.chatByName(title, sa.name);
			if (chat === null) continue;
			let previous: string | null = null;
			for (const message of rendered) {
				const sender = senderOf(message, chat, sa.name, previous);
				previous = sender;
				const label = labelOf(message);
				if (label === null) continue;
				model.recordExistingMessage(
					chat,
					sender,
					kindOf(message),
					label,
					[sa.name],
					{ deleted: message.deleted, reactions: message.reactions },
				);
			}
		}
	}
}

/** What identifies a rendered message in the model: its text, or the media
 *  name or duration its bubble shows. */
function labelOf(message: RenderedMessage): string | null {
	if (message.deleted) return null;
	if (message.photoAlts.length > 0) return message.photoAlts[0];
	if (message.fileName !== null) return message.fileName.trim();
	if (message.voiceDuration !== null) return message.voiceDuration;
	const text = message.text?.trim() ?? '';
	return text === '' ? null : text;
}

function kindOf(message: RenderedMessage): MessageKind {
	if (message.photoAlts.length > 0) return 'photo';
	if (message.fileName !== null) return 'file';
	if (message.voiceDuration !== null) return 'voice';
	return 'text';
}

/** Who sent a message the screen is showing: its reader when the bubble is
 *  its own, the name a group attributes it to, the sender of the bubble above
 *  it, or the peer of a direct chat. */
function senderOf(
	message: RenderedMessage,
	chat: ExpectedChat,
	reader: string,
	previous: string | null,
): string {
	if (message.mine) return reader;
	if (message.sender !== null) return message.sender;
	// A group renders the name on the first bubble of a run of consecutive
	// messages, so a bubble without one was sent by whoever sent the last.
	if (previous !== null) return previous;
	return chat.members.find(member => member !== reader) ?? reader;
}

function describeFound(found: FoundState[]): string {
	return found
		.filter(f => f.titles.length > 0)
		.map(f => `${f.sa.name} in ${f.titles.join(', ')}`)
		.join('; ');
}

/** One agent: preview features on, its device's notifications cleared, its
 *  add-contact link collected, and the members-less group a run that watches
 *  the connection chip reads it in. */
async function prepareAgent(
	model: ExpectedModel,
	sa: StressAgent,
	chipChat: string | null,
): Promise<void> {
	await sa.agent.enablePreviewFeatures();
	// The device keeps what earlier runs posted across the session's app data
	// reset, and the model starts with nothing on it.
	await sa.notifications?.clear();
	// A spec hands its agents over wherever its own setup left them, which for
	// `exchangeContacts` is their direct chat.
	await ensureHome(sa, model);
	sa.link = await contactLinkOf(sa.agent);
	if (chipChat === null) return;
	await createGroup(sa.agent, chipChat, []);
	await sa.agent.groupChatPage.back.click();
	await sa.agent.homePage.ready();
}

/** Park every hub and take the host's card off the test networks. */
async function parkHubs(real: Real): Promise<void> {
	for (const hub of real.hubs) await parkHub(hub);
	if (real.hubsNetwork !== null) await leaveWifi(real.hubsNetwork);
	real.hubsNetwork = null;
}

/** Leave nothing of a run behind: the cloud link healthy, hubs down, the
 *  host's card and the phones back on their usual networks. The test
 *  networks stay on the air, so the phones forget them or the supplicant
 *  may pick one again. */
async function teardown(real: Real): Promise<void> {
	if (mailboxDegradable()) await healMailboxLink();
	await parkHubs(real);
	if (real.networks.length === 0) return;
	for (const network of labNetworks(real)) await leaveWifi(network.ssid);
	await Promise.all(
		real.agents.map(async sa => {
			try {
				await restorePhone(sa, labNetworks(real));
			} catch {
				/* no saved network in range; nothing to restore */
			}
		}),
	);
}

/**
 * Put the cloud link and every app back to a known state before a sequence.
 * Chats and messages are left alone: they carry over as they do on the
 * devices, and so does what the model says each agent knows. What cannot
 * carry over is a state no move is guaranteed to undo — an agent backgrounded
 * in one sequence would stay away for the rest of the run while its device
 * piles up notifications nothing reads, and a degradation left in force would
 * outlive the sequence that raised it, so the heal that follows is checked
 * against a chip budget written for a short outage.
 */
async function resetSequence(model: ExpectedModel, real: Real): Promise<void> {
	if (mailboxDegradable()) {
		await healMailboxLink();
		model.setCloudUsable(await mailboxServing());
	}
	// Each agent's reset touches nothing but its own session.
	await Promise.all(real.agents.map(sa => resetAgent(model, sa)));
}

async function resetAgent(
	model: ExpectedModel,
	sa: StressAgent,
): Promise<void> {
	if (model.isStopped(sa.name)) {
		await sa.agent.startApp();
		model.startApp(sa.name);
	} else if (!model.isActive(sa.name)) {
		// Resumes onto the route it was taken away from, clearing what it was
		// showing for it.
		await sa.agent.startApp();
		model.foreground(sa.name);
	}
	if (!model.hasNetworks()) return;
	await sa.agent.disableWifi();
	model.agentLeave(sa.name);
}

/** A reported sequence, as the `move` builders that replay it. */
function asReproduction(moves: Iterable<{ toString(): string }>): string {
	return `[${[...moves].map(m => `move.${String(m)}`).join(', ')}]`;
}
