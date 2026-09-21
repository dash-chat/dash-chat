/** Moves that walk phones into and out of LANs, and sit still on them.
 *  They only apply to a run with networks; elsewhere their `check` is false
 *  and they are skipped. Each ends by asserting what the connection chips
 *  show, so a sequence fails at the exact move peer or hub discovery did
 *  not survive. */
import { type Real, at, byName, log, networkNamed } from '../agents';
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
		await openChat(actor, chipChat(m, actor.name), m);
		const { ssid, passphrase } = networkNamed(real, network);
		await actor.agent.connectWifi(ssid, passphrase);
		m.agentJoin(actor.name, network);
		await expectHubs(m, actor, `joining ${network}`);
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
		await openChat(actor, chipChat(m, actor.name), m);
		await actor.agent.disableWifi();
		m.agentLeave(actor.name);
		await expectHubs(m, actor, `leaving ${String(from)}`);
	}

	toString(): string {
		return `peerLeave(${this.agentIdx})`;
	}
}

/** How long a hub's mDNS records live in a phone's cache: a sit-still past
 *  it is what shows whether the app keeps them refreshed. */
export const MDNS_RECORD_TTL_S = 120;

export const networkMoves: Moves = [
	{ build: (a, n) => new PeerJoinMove(a, n), weight: 5 },
	{ build: a => new PeerLeaveMove(a), weight: 2 },
];

/** The network moves by the names a search prints them under. */
export const move = {
	peerJoin: (agentIdx: number, networkIdx: number): Move =>
		new PeerJoinMove(agentIdx, networkIdx),
	peerLeave: (agentIdx: number): Move => new PeerLeaveMove(agentIdx),
};
