/** Moves that walk phones into and out of LANs, and sit still on them.
 *  They only apply to a run with networks; elsewhere their `check` is false
 *  and they are skipped. Each ends by asserting what the connection chips
 *  show, so a sequence fails at the exact move peer or hub discovery did
 *  not survive. */
import fc from 'fast-check';

import {
	type Real,
	type StressAgent,
	at,
	byName,
	goHome,
	log,
	networkNamed,
} from '../agents';
import { chipChat, expectHubs, openChat } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Walk a phone into a LAN with the app on screen. The chip is on screen
 *  before the move, so the budget runs from the move alone. */
class PeerJoinMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly networkIdx: number,
	) {
		super();
	}

	private movable(m: Readonly<ExpectedModel>) {
		return m.activeNames().filter(n => m.otherNetworks(n).length > 0);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(this.movable(m), this.agentIdx));
		const network = at(m.otherNetworks(actor.name), this.networkIdx);
		log(`${actor.name}: ${this.toString()} -> joins ${network}`);
		const page = await openChat(actor, chipChat(m, actor.name), m);
		const { ssid, passphrase } = networkNamed(real, network);
		await actor.agent.connectWifi(ssid, passphrase);
		m.agentJoin(actor.name, network);
		await expectHubs(m, actor, `joining ${network}`);
		await goHome(actor, page);
	}

	toString(): string {
		return `peerJoin(${this.agentIdx},${this.networkIdx})`;
	}
}

/** Walk a phone out of its LAN, off the air, with the app on screen. */
class PeerLeaveMove extends Move {
	constructor(readonly agentIdx: number) {
		super();
	}

	private movable(m: Readonly<ExpectedModel>) {
		return m.activeNames().filter(n => m.networkOf(n) !== null);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(this.movable(m), this.agentIdx));
		const from = m.networkOf(actor.name);
		log(`${actor.name}: ${this.toString()} -> leaves ${String(from)}`);
		const page = await openChat(actor, chipChat(m, actor.name), m);
		await actor.agent.disableWifi();
		m.agentLeave(actor.name);
		await expectHubs(m, actor, `leaving ${String(from)}`);
		await goHome(actor, page);
	}

	toString(): string {
		return `peerLeave(${this.agentIdx})`;
	}
}

/** How long a phone may sit without an automation command before the device
 *  idles its UiAutomator2 / WebDriver session out from under us. */
const SESSION_IDLE_LIMIT_MS = 20_000;

/** Pause `ms` while touching each phone's session within
 *  [`SESSION_IDLE_LIMIT_MS`]: a bare `pause` sends no command, so a long
 *  sit-still lets the session go stale and the next move finds it gone. */
async function idle(
	real: Real,
	agents: StressAgent[],
	ms: number,
): Promise<void> {
	const until = Date.now() + ms;
	while (Date.now() < until) {
		await real.agents[0].agent.pause(
			Math.min(SESSION_IDLE_LIMIT_MS, until - Date.now()),
		);
		for (const sa of agents) await sa.agent.execute(() => true);
	}
}

/** How long a hub's mDNS records live in a phone's cache: a sit-still past
 *  it is what shows whether the app keeps them refreshed. */
const MDNS_TTL_S = 120;

/** Sit still with the chats open, then every driveable phone must still show
 *  exactly the hubs on its LAN. */
class SleepMove extends Move {
	constructor(readonly seconds: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasNetworks();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		const watching = m.activeNames().map(name => byName(real, name));
		for (const sa of watching) await openChat(sa, chipChat(m, sa.name), m);
		await idle(real, watching, this.seconds * 1_000);
		for (const sa of watching) {
			await expectHubs(m, sa, `sleeping ${this.seconds}s`);
			await goHome(sa, sa.agent.groupChatPage);
		}
	}

	toString(): string {
		return `sleep(${this.seconds})`;
	}
}

export const networkMoves: Moves = [
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, n]) => new PeerJoinMove(a, n)),
		weight: 5,
	},
	{ arbitrary: fc.nat().map(a => new PeerLeaveMove(a)), weight: 2 },
	{
		arbitrary: fc
			.integer({ min: 1, max: MDNS_TTL_S + 10 })
			.map(s => new SleepMove(s)),
		weight: 1,
	},
];

/** The network moves by the names a search prints them under. */
export const move = {
	peerJoin: (agentIdx: number, networkIdx: number): Move =>
		new PeerJoinMove(agentIdx, networkIdx),
	peerLeave: (agentIdx: number): Move => new PeerLeaveMove(agentIdx),
	sleep: (seconds: number): Move => new SleepMove(seconds),
};
