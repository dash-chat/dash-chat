/**
 * The system under test around a model-based run: spawning it with the agents
 * ready, and running the property over sequences a spec draws with
 * fast-check's own arbitraries, leaving nothing behind.
 */
import fc from 'fast-check';

import { stopHotspot } from '../../setup/hotspot';
import { stopLocalHub } from '../../setup/local-hub';
import type { Agent } from '../../setup/setup-agents';
import { navigateToAddContact } from '../flows/exchange-contacts';
import { createGroup } from '../flows/exchange-contacts-and-create-group';
import {
	type Real,
	addContact,
	ensureHome,
	goHome,
	newReal,
	openChat,
} from './agents';
import type { Cmd } from './commands';
import { type ExpectedModel, newModel } from './model';
import { verifyConvergence } from './verify';

/** fast-check's model and real, as one system under test. */
export interface System {
	model: ExpectedModel;
	real: Real;
}

/**
 * The model and real for a run, with the agents driven to where commands
 * expect them: preview features on, each one's contact link collected, and —
 * given Wi-Fi cards to raise LANs on — a members-less group chat each, to
 * read the connection chip in. Every agent is left on its home page.
 */
export async function setupFastCheck(init: {
	agents: { agent: Agent; name: string }[];
	wifiDevices?: string[];
}): Promise<System> {
	const real = newReal(init);
	const model = newModel(real);
	await prepareAgents(model, real);
	return { model, real };
}

async function prepareAgents(model: ExpectedModel, real: Real): Promise<void> {
	for (const sa of real.agents) {
		await sa.agent.enablePreviewFeatures();
		await sa.agent.homePage.ready();
		await navigateToAddContact(sa.agent);
		sa.link = await sa.agent.addContactPage.getAddContactLink();
		await sa.agent.addContactPage.back.click();
		await sa.agent.newMessagePage.back.click();
		await sa.agent.homePage.ready();
		if (model.networkCapacity === 0) continue;
		const chatName = model.nextGroupName();
		await createGroup(sa.agent, chatName, []);
		await ensureHome(sa);
		model.addGroup(sa.name, [], chatName);
	}
	if (model.networkCapacity === 0) return;
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
			await openChat(sa, model.directChat(sa.name, peer.name), model);
			await sa.agent.directChatPage.waitForPeerProfile();
			await goHome(sa, sa.agent.directChatPage);
		}
	}
}

/** Take down every hub process and every LAN the run raised. */
async function stopNetworks(real: Real): Promise<void> {
	for (const hub of real.hubs) {
		if (hub.process === null) continue;
		await stopLocalHub(hub.process);
		hub.process = null;
	}
	for (const network of real.networks) stopHotspot(network.ssid);
	real.networks.length = 0;
}

/**
 * Put the network side back to its starting state — no LAN up, every hub
 * parked, every phone foregrounded, at home and off the air — so that a
 * fresh draw and a shrinking replay begin from the same place. Chats and
 * messages are left alone: they carry over as they do on the devices.
 */
async function resetNetworks(model: ExpectedModel, real: Real): Promise<void> {
	if (model.networkCapacity === 0) return;
	await stopNetworks(real);
	for (const hub of model.hubs) hub.network = null;
	for (const name of model.networkNames()) model.killNetwork(name);
	for (const sa of real.agents) {
		await sa.agent.startApp();
		model.foreground(sa.name);
		await ensureHome(sa);
		await sa.agent.disableWifi();
		model.agentLeave(sa.name);
	}
}

/** Leave nothing of a run behind: LANs and hubs down, and the phones back on
 *  the air so they return to their usual network. */
async function teardown(real: Real): Promise<void> {
	await stopNetworks(real);
	if (real.wifiDevices.length === 0) return;
	for (const sa of real.agents) {
		try {
			await sa.agent.enableWifi();
		} catch {
			/* no saved network in range; nothing to restore */
		}
	}
}

/** A reported sequence, as the `command` builders that replay it. */
function asRegression(commands: Iterable<{ toString(): string }>): string {
	return `[${[...commands].map(c => `command.${String(c)}`).join(', ')}]`;
}

/**
 * Run the property "any sequence drawn from `sequences` keeps the real system
 * matching the model": the network side is reset before each sequence, and
 * after it whatever the moves left unsynced has to catch up once the phones
 * are back on their usual LAN together, so everything is torn down and the
 * end state verified. `params` are fast-check's own (`numRuns`, `seed`,
 * `interruptAfterTimeLimit`, …). A failure throws fast-check's report plus
 * the shrunk sequence as `command` builders, ready to paste into a
 * regression spec.
 */
export async function run(
	{ model, real }: System,
	sequences: fc.Arbitrary<Iterable<Cmd>>,
	params: fc.Parameters<[Iterable<Cmd>]>,
): Promise<void> {
	let out: fc.RunDetails<[Iterable<Cmd>]>;
	try {
		out = await fc.check(
			fc.asyncProperty(sequences, async cmds => {
				await fc.asyncModelRun(async () => {
					await resetNetworks(model, real);
					return { model, real };
				}, cmds);
				await teardown(real);
				await verifyConvergence(model, real);
			}),
			params,
		);
	} finally {
		// A sequence that failed mid-way skipped its own teardown.
		await teardown(real);
	}
	if (out.failed) {
		const commands = out.counterexample?.[0];
		throw new Error(
			`${fc.defaultReportMessage(out)}\n\nRegression: ` +
				(commands === undefined ? 'none reported' : asRegression(commands)),
		);
	}
}
