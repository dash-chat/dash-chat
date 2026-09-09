/**
 * The random "normal user" command pool. Each command drives one behavior a
 * real user would — adding a contact, messaging, reacting, backgrounding the
 * app, walking into another network — through one agent's UI, records the
 * expected outcome in the ExpectedModel, and returns to the home page, so any
 * generated sequence is valid.
 *
 * Commands carry abstract indices (resolved modulo the eligible options at
 * run time), the canonical fast-check pattern for targets that only exist
 * once earlier commands have run.
 *
 * The network commands — raising and killing LANs, creating hubs, moving hubs
 * and phones between LANs, sleeping — only apply to a run with a network
 * capacity; elsewhere their `check` is false and they are skipped. Each of
 * them ends by asserting what the connection chip shows and, for every
 * contact, whether a text crosses — so a sequence fails at the exact move hub
 * discovery or peer discovery did not survive.
 */
import fc from 'fast-check';

import { allocateFreePort } from '../../setup/allocate-port';
import { type Hotspot, startHotspot, stopHotspot } from '../../setup/hotspot';
import { spawnLocalHub, stopLocalHub } from '../../setup/local-hub';
import { SYNC_TIMEOUT } from '../timeouts';
import {
	type HubReal,
	QUICK_EMOJIS,
	type Real,
	type StressAgent,
	addContact,
	at,
	byName,
	goHome,
	log,
	openChat,
} from './agents';
import { type ExpectedChat, ExpectedModel } from './model';
import { verifyConvergence } from './verify';

export type Cmd = fc.AsyncCommand<ExpectedModel, Real>;

/** What any move gets before its effect has to be on screen — a hub
 *  appearing or disappearing, every hub named in the dialog, a peer on the
 *  same LAN acking a text. Beyond this a user reads the app as slow or
 *  broken. */
export const DISCOVERY_MS = 2_000;

/** Long enough for a probe that could cross to have crossed, so one still
 *  unacked afterwards means the pair really is cut off. */
const CUT_OFF_MS = 10_000;

/** Hubs a run may create: each is a process on the host. */
const MAX_HUBS = 2;

class AddContactCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly peerIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.notYetAdded(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.notYetAdded(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const peer = byName(real, at(m.notYetAdded(actor.name), this.peerIdx));
		log(`${actor.name}: ${this.toString()} -> adds ${peer.name}`);
		await addContact(actor, peer);
		m.recordAdded(actor.name, peer.name);
	}

	toString(): string {
		return `addContact(${this.agentIdx},${this.peerIdx})`;
	}
}

class SendTextCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.chatsFor(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.chatsFor(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const chat = at(m.chatsFor(actor.name), this.chatIdx);
		const label = m.nextLabel(actor.name);
		log(
			`${actor.name}: ${this.toString()} -> ${label} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		await page.composer.sendMessage(label);
		m.addMessage(chat, actor.name, 'text', label);
		await goHome(actor, page);
	}

	toString(): string {
		return `sendText(${this.agentIdx},${this.chatIdx})`;
	}
}

class SendPhotoCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.chatsFor(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.chatsFor(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const chat = at(m.chatsFor(actor.name), this.chatIdx);
		const label = m.nextLabel(actor.name);
		log(
			`${actor.name}: ${this.toString()} -> ${label} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		// Direct chats mount the composer only once the chat leaves the pending
		// state, which needs the peer's profile to have synced.
		await page.composer.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		await page.composer.attachPhotos(label);
		await page.composer.send();
		await page.messages.waitForPhotoMessage(label);
		m.addMessage(chat, actor.name, 'photo', label);
		await goHome(actor, page);
	}

	toString(): string {
		return `sendPhoto(${this.agentIdx},${this.chatIdx})`;
	}
}

class CreateGroupCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly offsetIdx: number,
		readonly countIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.contactsOf(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.contactsOf(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const contacts = m.contactsOf(actor.name);
		const count = 1 + (this.countIdx % contacts.length);
		const members = Array.from(
			{ length: count },
			(_, i) => contacts[(this.offsetIdx + i) % contacts.length],
		);
		const name = m.nextGroupName();
		log(
			`${actor.name}: ${this.toString()} -> ${name} with ${members.join(',')}`,
		);
		const { agent } = actor;
		await agent.homePage.ready();
		await agent.homePage.newMessageButton.click();
		await agent.newMessagePage.ready();
		await agent.newMessagePage.newGroup.click();
		await agent.newGroupPage.addMembersStep.ready();
		for (const member of members) {
			await agent.newGroupPage.addMembersStep.addContactByName(member);
		}
		await agent.newGroupPage.addMembersStep.nextButton.click();
		await agent.newGroupPage.groupInfoStep.ready();
		await agent.newGroupPage.groupInfoStep.setName(name);
		await agent.newGroupPage.groupInfoStep.createButton.click();
		await agent.groupChatPage.ready();
		m.addGroup(actor.name, members, name);
		await goHome(actor, agent.groupChatPage);
	}

	toString(): string {
		return `createGroup(${this.agentIdx},${this.offsetIdx},${this.countIdx})`;
	}
}

class ReactCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
		readonly emojiIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name),
			this.targetIdx,
		);
		// Re-reacting with the current emoji toggles the reaction off; always
		// picking a different one keeps the expected state a plain "has emoji".
		const emoji = at(
			QUICK_EMOJIS.filter(e => e !== message.reactions.get(actor.name)),
			this.emojiIdx,
		);
		log(`${actor.name}: ${this.toString()} -> ${emoji} on ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.reactWith(emoji);
		message.reactions.set(actor.name, emoji);
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `react(${this.agentIdx},${this.targetIdx},${this.emojiIdx})`;
	}
}

class ReplyCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name),
			this.targetIdx,
		);
		const label = m.nextLabel(actor.name);
		log(`${actor.name}: ${this.toString()} -> ${label} to ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.reply(label);
		message.hasReply = true;
		m.addMessage(chat, actor.name, 'text', label, message.label);
		await goHome(actor, page);
	}

	toString(): string {
		return `reply(${this.agentIdx},${this.targetIdx})`;
	}
}

class EditCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n, true).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n, true).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name, true),
			this.targetIdx,
		);
		const newText = `${message.label} v${message.edits + 1}`;
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.edit(message.text, newText);
		message.edits += 1;
		message.text = newText;
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `edit(${this.agentIdx},${this.targetIdx})`;
	}
}

class DeleteCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	private targets(m: Readonly<ExpectedModel>, name: string) {
		return m.interactionTargets(name, true).filter(t => !t.message.hasReply);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => this.targets(m, n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => this.targets(m, n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(this.targets(m, actor.name), this.targetIdx);
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.deleteForEveryone();
		message.deleted = true;
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `delete(${this.agentIdx},${this.targetIdx})`;
	}
}

/** Backgrounds an agent and leaves it backgrounded: later commands keep
 * acting through the other agents (including sending to this one), and a
 * ForegroundCommand — or the next convergence check — brings it back, making
 * catch-up-after-background part of every run. */
class BackgroundCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeMobileNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.activeMobileNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.backgroundApp();
		m.background(actor.name);
	}

	toString(): string {
		return `background(${this.agentIdx})`;
	}
}

class ForegroundCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.backgroundedNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.backgroundedNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.startApp();
		await actor.agent.homePage.ready();
		m.foreground(actor.name);
		if (m.networkCapacity > 0) {
			await checkHubs(m, actor, 'coming back to the foreground');
			await checkPeers(m, actor, 'coming back to the foreground');
		}
	}

	toString(): string {
		return `foreground(${this.agentIdx})`;
	}
}

class RestartCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.activeNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		if (actor.agent.platform === 'desktop') {
			await actor.agent.restart();
		} else {
			// Not restart(): a new Appium session fast-resets (pm clear) on
			// Android, wiping the profile. Stop + activate keeps the data dir.
			await actor.agent.stopApp();
			await actor.agent.startApp();
		}
		await actor.agent.homePage.ready();
		if (m.networkCapacity > 0) {
			await checkHubs(m, actor, 'the app restarted');
			await checkPeers(m, actor, 'the app restarted');
		}
	}

	toString(): string {
		return `restart(${this.agentIdx})`;
	}
}

class CheckpointCommand implements Cmd {
	check(): boolean {
		return true;
	}

	async run(model: ExpectedModel, real: Real): Promise<void> {
		await verifyConvergence(model, real);
	}

	toString(): string {
		return 'checkpoint()';
	}
}

function networkNamed(real: Real, name: string): Hotspot {
	const found = real.networks.find(n => n.ssid === name);
	if (found === undefined) throw new Error(`no network named ${name}`);
	return found;
}

function hubNamed(real: Real, name: string): HubReal {
	const found = real.hubs.find(h => h.name === name);
	if (found === undefined) throw new Error(`no hub named ${name}`);
	return found;
}

/** The members-less group every agent of a run with networks gets: the page
 *  its connection chip is read on. */
function chipChat(m: ExpectedModel, name: string): ExpectedChat {
	const chat = m
		.chatsFor(name)
		.find(c => c.kind === 'group' && c.members.length === 1);
	if (chat === undefined) {
		throw new Error(`${name} has no chat to read its chip in`);
	}
	return chat;
}

/** Wait until `sa`'s chip shows the hubs the model says its network has.
 *  Assumes the agent is on its chip chat. */
async function expectHubs(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	const chip = sa.agent.groupChatPage.connectionStatusIndicator;
	const expected = m.expectedHubs(sa.name);
	if (expected === 0) {
		await chip.waitForStatus(
			'disconnected',
			DISCOVERY_MS,
			`${sa.name}: the chip still showed a hub ${DISCOVERY_MS / 1_000}s after ${after}`,
		);
		return;
	}
	await chip.waitForStatus(
		'local',
		DISCOVERY_MS,
		`${sa.name}: the chip did not read local within ${DISCOVERY_MS / 1_000}s after ${after}`,
	);
	await chip.waitForLocalHubCount(
		expected,
		DISCOVERY_MS,
		`${sa.name}: the dialog did not name ${expected} hub(s) after ${after}`,
	);
}

/** Open `sa`'s chip chat, check the chip against the model, and return home. */
async function checkHubs(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	const page = await openChat(sa, chipChat(m, sa.name), m);
	await expectHubs(m, sa, after);
	await goHome(sa, page);
}

/** Probe every contact of `sa` with a text in their direct chat. A peer the
 *  model says shares `sa`'s LAN must ack it — "delivered" on `sa`'s side —
 *  within [`DISCOVERY_MS`]; one it says is apart must not have acked it after
 *  [`CUT_OFF_MS`]. A backgrounded peer is not held to the budget: catching up
 *  is verified at convergence. Every probe stays in the model as a message. */
async function checkPeers(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	for (const peer of m.contactsOf(sa.name)) {
		const chat = m.directChat(sa.name, peer);
		const label = m.nextLabel(sa.name);
		const page = await openChat(sa, chat, m);
		await page.composer.sendMessage(label);
		m.addMessage(chat, sa.name, 'text', label);
		if (m.reachable(sa.name, peer) && !m.isBackgrounded(peer)) {
			try {
				await page.messages.waitForMessageStatus(
					label,
					['delivered'],
					DISCOVERY_MS,
				);
			} catch {
				throw new Error(
					`${sa.name}: ${peer} on the same LAN never acked a text within ` +
						`${DISCOVERY_MS / 1_000}s after ${after}`,
				);
			}
		} else if (!m.reachable(sa.name, peer)) {
			await sa.agent.pause(CUT_OFF_MS);
			if ((await page.messages.messageStatusFor(label)) === 'delivered') {
				throw new Error(
					`${sa.name}: ${peer} acked a text after ${after} although the ` +
						'model has them on different networks; the LANs are not isolated',
				);
			}
		}
		await goHome(sa, page);
	}
}

/** Every driveable agent on `network` checks its chip. */
async function checkHubsOn(
	m: ExpectedModel,
	real: Real,
	network: string,
	after: string,
): Promise<void> {
	for (const name of m.activeNamesOn(network)) {
		await checkHubs(m, byName(real, name), after);
	}
}

/** Park a hub's process: the hub keeps its identity for a later join. */
async function parkHub(hub: HubReal): Promise<void> {
	if (hub.process === null) return;
	await stopLocalHub(hub.process);
	hub.process = null;
}

/** Raise a LAN on a free Wi-Fi card. */
class CreateNetworkCommand implements Cmd {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.canCreateNetwork();
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const device = real.wifiDevices.find(
			d => !real.networks.some(n => n.device === d),
		);
		if (device === undefined) throw new Error('no Wi-Fi card is free');
		const network = m.createNetwork();
		log(`${this.toString()} -> ${network.name} on ${device}`);
		real.networks.push(await startHotspot(network.name, device));
	}

	toString(): string {
		return 'createNetwork()';
	}
}

/** Take a LAN down under whoever is on it. Its hubs are left parked, on no
 *  network; its phones are left on no network, and must show no hub. */
class KillNetworkCommand implements Cmd {
	constructor(readonly networkIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.networks.length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const name = at(m.networkNames(), this.networkIdx).valueOf();
		log(`${this.toString()} -> kills ${name}`);
		const orphans = m.activeNamesOn(name).map(n => byName(real, n));
		stopHotspot(name);
		real.networks.splice(
			real.networks.findIndex(n => n.ssid === name),
			1,
		);
		for (const hub of m.hubs) {
			if (hub.network === name) await parkHub(hubNamed(real, hub.name));
		}
		// Off the air rather than wherever the supplicant would fall back to:
		// a network outside the model would be invisible to it.
		for (const sa of orphans) await sa.agent.disableWifi();
		m.killNetwork(name);
		for (const sa of orphans) {
			await checkHubs(m, sa, `${name} was killed under it`);
			await checkPeers(m, sa, `${name} was killed under it`);
		}
	}

	toString(): string {
		return `killNetwork(${this.networkIdx})`;
	}
}

/** Give a hub an identity — db, key, port — without putting it anywhere. */
class CreateHubCommand implements Cmd {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.networkCapacity > 0 && m.hubs.length < MAX_HUBS;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const hub = m.createHub();
		log(`${this.toString()} -> ${hub.name}`);
		real.hubs.push({
			name: hub.name,
			port: await allocateFreePort(),
			process: null,
		});
	}

	toString(): string {
		return 'createHub()';
	}
}

/** Put a hub on a LAN it is not on — bringing it up there, or moving it
 *  from where it was. Phones on that LAN must show it; phones on the LAN it
 *  left must drop it. */
class HubJoinCommand implements Cmd {
	constructor(
		readonly hubIdx: number,
		readonly networkIdx: number,
	) {}

	private movable(m: Readonly<ExpectedModel>) {
		return m.hubs.filter(h => m.networkNames().some(n => n !== h.network));
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const hub = at(this.movable(m), this.hubIdx);
		const network = at(
			m.networkNames().filter(n => n !== hub.network),
			this.networkIdx,
		);
		const from = hub.network;
		log(`${this.toString()} -> ${hub.name} joins ${network}`);
		const hr = hubNamed(real, hub.name);
		await parkHub(hr);
		hr.process = await spawnLocalHub(
			hub.name,
			networkNamed(real, network).address,
			hr.port,
		);
		hub.network = network;
		await checkHubsOn(m, real, network, `${hub.name} joined ${network}`);
		if (from !== null) {
			await checkHubsOn(m, real, from, `${hub.name} left ${from}`);
		}
	}

	toString(): string {
		return `hubJoin(${this.hubIdx},${this.networkIdx})`;
	}
}

class HubLeaveCommand implements Cmd {
	constructor(readonly hubIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.hubs.some(h => h.network !== null);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const hub = at(
			m.hubs.filter(h => h.network !== null),
			this.hubIdx,
		);
		const from = hub.network;
		if (from === null) throw new Error(`${hub.name} is on no network`);
		log(`${this.toString()} -> ${hub.name} leaves ${from}`);
		await parkHub(hubNamed(real, hub.name));
		hub.network = null;
		await checkHubsOn(m, real, from, `${hub.name} left ${from}`);
	}

	toString(): string {
		return `hubLeave(${this.hubIdx})`;
	}
}

/** Walk a phone into a LAN with the app on screen. The chip is on screen
 *  before the move, so the budget runs from the move alone. */
class PeerJoinCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly networkIdx: number,
	) {}

	private movable(m: Readonly<ExpectedModel>) {
		return m.activeNames().filter(n => m.otherNetworks(n).length > 0);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(this.movable(m), this.agentIdx));
		const network = at(m.otherNetworks(actor.name), this.networkIdx);
		log(`${actor.name}: ${this.toString()} -> joins ${network}`);
		const page = await openChat(actor, chipChat(m, actor.name), m);
		const { ssid, passphrase } = networkNamed(real, network);
		await actor.agent.connectWifi(ssid, passphrase);
		m.agentJoin(actor.name, network);
		await expectHubs(m, actor, `joining ${network}`);
		await goHome(actor, page);
		await checkPeers(m, actor, `joining ${network}`);
	}

	toString(): string {
		return `peerJoin(${this.agentIdx},${this.networkIdx})`;
	}
}

/** Walk a phone out of its LAN, off the air, with the app on screen. */
class PeerLeaveCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	private movable(m: Readonly<ExpectedModel>) {
		return m.activeNames().filter(n => m.networkOf(n) !== null);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(this.movable(m), this.agentIdx));
		const from = m.networkOf(actor.name);
		log(`${actor.name}: ${this.toString()} -> leaves ${String(from)}`);
		const page = await openChat(actor, chipChat(m, actor.name), m);
		await actor.agent.disableWifi();
		m.agentLeave(actor.name);
		await expectHubs(m, actor, `leaving ${String(from)}`);
		await goHome(actor, page);
		await checkPeers(m, actor, `leaving ${String(from)}`);
	}

	toString(): string {
		return `peerLeave(${this.agentIdx})`;
	}
}

/** Sit still with the chats open, then every driveable phone must still show
 *  exactly the hubs on its LAN. */
class SleepCommand implements Cmd {
	constructor(readonly seconds: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.networkCapacity > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		const watching = m.activeNames().map(name => byName(real, name));
		for (const sa of watching) await openChat(sa, chipChat(m, sa.name), m);
		await real.agents[0].agent.pause(this.seconds * 1_000);
		for (const sa of watching) {
			await expectHubs(m, sa, `sleeping ${this.seconds}s`);
			await goHome(sa, sa.agent.groupChatPage);
		}
		for (const sa of watching) {
			await checkPeers(m, sa, `sleeping ${this.seconds}s`);
		}
	}

	toString(): string {
		return `sleep(${this.seconds})`;
	}
}

const backgroundArbitrary = fc.nat().map(a => new BackgroundCommand(a));
const foregroundArbitrary = fc.nat().map(a => new ForegroundCommand(a));
const restartArbitrary = fc.nat().map(a => new RestartCommand(a));

const createNetworkArbitrary = fc.constant(new CreateNetworkCommand());
const killNetworkArbitrary = fc.nat().map(n => new KillNetworkCommand(n));
const createHubArbitrary = fc.constant(new CreateHubCommand());
const hubJoinArbitrary = fc
	.tuple(fc.nat(), fc.nat())
	.map(([h, n]) => new HubJoinCommand(h, n));
const hubLeaveArbitrary = fc.nat().map(h => new HubLeaveCommand(h));
const peerJoinArbitrary = fc
	.tuple(fc.nat(), fc.nat())
	.map(([a, n]) => new PeerJoinCommand(a, n));
const peerLeaveArbitrary = fc.nat().map(a => new PeerLeaveCommand(a));
const sleepArbitrary = fc
	.integer({ min: 1, max: 300 })
	.map(s => new SleepCommand(s));

/** One move a normal user makes, weighted as a day of use is. */
export const userCommand: fc.Arbitrary<Cmd> = fc.oneof(
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, p]) => new AddContactCommand(a, p)),
		weight: 10,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, c]) => new SendTextCommand(a, c)),
		weight: 10,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, c]) => new SendPhotoCommand(a, c)),
		weight: 4,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat(), fc.nat())
			.map(([a, o, c]) => new CreateGroupCommand(a, o, c)),
		weight: 2,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat(), fc.nat())
			.map(([a, t, e]) => new ReactCommand(a, t, e)),
		weight: 5,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new ReplyCommand(a, t)),
		weight: 3,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new EditCommand(a, t)),
		weight: 3,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new DeleteCommand(a, t)),
		weight: 2,
	},
	{ arbitrary: backgroundArbitrary, weight: 3 },
	{ arbitrary: foregroundArbitrary, weight: 3 },
	{ arbitrary: restartArbitrary, weight: 1 },
	{ arbitrary: fc.constant(new CheckpointCommand()), weight: 2 },
);

/** One move that makes or breaks hub discovery, and nothing else. */
export const networkCommand: fc.Arbitrary<Cmd> = fc.oneof(
	{ arbitrary: createNetworkArbitrary, weight: 3 },
	{ arbitrary: killNetworkArbitrary, weight: 1 },
	{ arbitrary: createHubArbitrary, weight: 2 },
	{ arbitrary: hubJoinArbitrary, weight: 4 },
	{ arbitrary: hubLeaveArbitrary, weight: 2 },
	{ arbitrary: peerJoinArbitrary, weight: 5 },
	{ arbitrary: peerLeaveArbitrary, weight: 2 },
	{ arbitrary: sleepArbitrary, weight: 3 },
	{ arbitrary: backgroundArbitrary, weight: 1 },
	{ arbitrary: foregroundArbitrary, weight: 1 },
	{ arbitrary: restartArbitrary, weight: 1 },
);

/** The discovery commands by the names a search prints them under, so a
 *  reported sequence can be pasted into a regression spec as is. */
export const command = {
	createNetwork: (): Cmd => new CreateNetworkCommand(),
	killNetwork: (networkIdx: number): Cmd => new KillNetworkCommand(networkIdx),
	createHub: (): Cmd => new CreateHubCommand(),
	hubJoin: (hubIdx: number, networkIdx: number): Cmd =>
		new HubJoinCommand(hubIdx, networkIdx),
	hubLeave: (hubIdx: number): Cmd => new HubLeaveCommand(hubIdx),
	peerJoin: (agentIdx: number, networkIdx: number): Cmd =>
		new PeerJoinCommand(agentIdx, networkIdx),
	peerLeave: (agentIdx: number): Cmd => new PeerLeaveCommand(agentIdx),
	sleep: (seconds: number): Cmd => new SleepCommand(seconds),
	background: (agentIdx: number): Cmd => new BackgroundCommand(agentIdx),
	foreground: (agentIdx: number): Cmd => new ForegroundCommand(agentIdx),
	restart: (agentIdx: number): Cmd => new RestartCommand(agentIdx),
};
