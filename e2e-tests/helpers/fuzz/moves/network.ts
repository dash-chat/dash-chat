/** Moves that make and break LANs, and walk phones into and out of them.
 *  They only apply to a run with a network capacity; elsewhere their `check`
 *  is false and they are skipped. Each ends by asserting what the connection
 *  chips show, so a sequence fails at the exact move peer or hub discovery
 *  did not survive. */
import fc from 'fast-check';

import { startHotspot, stopHotspot } from '../../../setup/hotspot';
import {
	type Real,
	at,
	byName,
	goHome,
	hubNamed,
	log,
	networkNamed,
	parkHub,
} from '../agents';
import { checkHubs, chipChat, expectHubs, openChat } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Raise a LAN on a free Wi-Fi card. */
class CreateNetworkMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.canCreateNetwork();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
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
class KillNetworkMove extends Move {
	constructor(readonly networkIdx: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.networks.length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
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
		}
	}

	toString(): string {
		return `killNetwork(${this.networkIdx})`;
	}
}

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

/** Sit still with the chats open, then every driveable phone must still show
 *  exactly the hubs on its LAN. */
class SleepMove extends Move {
	constructor(readonly seconds: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.networkCapacity > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		const watching = m.activeNames().map(name => byName(real, name));
		for (const sa of watching) await openChat(sa, chipChat(m, sa.name), m);
		await real.agents[0].agent.pause(this.seconds * 1_000);
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
	{ arbitrary: fc.constant(new CreateNetworkMove()), weight: 3 },
	{ arbitrary: fc.nat().map(n => new KillNetworkMove(n)), weight: 1 },
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, n]) => new PeerJoinMove(a, n)),
		weight: 5,
	},
	{ arbitrary: fc.nat().map(a => new PeerLeaveMove(a)), weight: 2 },
	{
		arbitrary: fc.integer({ min: 1, max: 300 }).map(s => new SleepMove(s)),
		weight: 3,
	},
];

/** The network moves by the names a search prints them under. */
export const move = {
	createNetwork: (): Move => new CreateNetworkMove(),
	killNetwork: (networkIdx: number): Move => new KillNetworkMove(networkIdx),
	peerJoin: (agentIdx: number, networkIdx: number): Move =>
		new PeerJoinMove(agentIdx, networkIdx),
	peerLeave: (agentIdx: number): Move => new PeerLeaveMove(agentIdx),
	sleep: (seconds: number): Move => new SleepMove(seconds),
};
