/** Moves a local hub makes: coming into being, its process starting,
 *  stopping or dying, and joining or leaving a LAN. Every hub is a process
 *  on the host, whose one Wi-Fi card is the hubs' location, so a join or a
 *  leave moves all of them at once. Each ends by asserting what the
 *  connection chips show, so a sequence fails at the exact move hub
 *  discovery did not survive. */
import fc from 'fast-check';

import { allocateFreePort } from '../../../setup/allocate-port';
import { joinWifi, leaveWifi } from '../../../setup/host-wifi';
import { spawnLocalHub } from '../../../setup/local-hub';
import { type Real, at, hubNamed, log, networkNamed, parkHub } from '../agents';
import { checkHubsOn } from '../checks';
import type { ExpectedHub, ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Hubs a run may create: each is a process on the host. */
const MAX_HUBS = 2;

/** Give a hub an identity — db, key, port — without running it. */
class CreateHubMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasNetworks() && m.hubs.length < MAX_HUBS;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
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

/** Every driveable phone on the hubs' LAN checks its chip, if they are on one. */
async function checkHubsLan(
	m: ExpectedModel,
	real: Real,
	after: string,
): Promise<void> {
	if (real.hubsNetwork === null) return;
	await checkHubsOn(m, real, real.hubsNetwork, after);
}

/** Bring a hub's process up. Phones on its LAN must show it. */
class StartHubMove extends Move {
	constructor(readonly hubIdx: number) {
		super();
	}

	private startable(m: Readonly<ExpectedModel>): ExpectedHub[] {
		return m.hubs.filter(h => !h.running);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.startable(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const hub = at(this.startable(m), this.hubIdx);
		log(`${this.toString()} -> ${hub.name}`);
		const hr = hubNamed(real, hub.name);
		hr.process = await spawnLocalHub(hub.name, hr.port);
		m.startHub(hub.name);
		await checkHubsLan(m, real, `${hub.name} started`);
	}

	toString(): string {
		return `startHub(${this.hubIdx})`;
	}
}

/** Take a hub's process down, with or without its mDNS goodbye. Phones on
 *  its LAN must drop it either way. */
class StopHubMove extends Move {
	constructor(
		readonly hubIdx: number,
		readonly signal: 'SIGINT' | 'SIGKILL',
	) {
		super();
	}

	private stoppable(m: Readonly<ExpectedModel>): ExpectedHub[] {
		return m.hubs.filter(h => h.running);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.stoppable(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const hub = at(this.stoppable(m), this.hubIdx);
		log(`${this.toString()} -> ${hub.name}`);
		await parkHub(hubNamed(real, hub.name), this.signal);
		m.stopHub(hub.name);
		await checkHubsLan(
			m,
			real,
			`${hub.name} ${this.signal === 'SIGKILL' ? 'was killed' : 'stopped'}`,
		);
	}

	toString(): string {
		return `${this.signal === 'SIGKILL' ? 'killHub' : 'stopHub'}(${this.hubIdx})`;
	}
}

/** Put the hubs on a LAN they are not on — the host's card joining it —
 *  bringing every hub along. Phones there must show the running ones;
 *  phones on the LAN they left must drop them. */
class HubJoinMove extends Move {
	constructor(readonly networkIdx: number) {
		super();
	}

	/** Every hub shares the card, so any one says where they all are. */
	private elsewhere(m: Readonly<ExpectedModel>): string[] {
		if (m.hubs.length === 0) return [];
		return m.networkNames().filter(n => n !== m.hubs[0].network);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.elsewhere(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const from = real.hubsNetwork;
		const network = at(this.elsewhere(m), this.networkIdx);
		log(`${this.toString()} -> hubs join ${network}`);
		if (real.hubsDevice === null) throw new Error('the host has no Wi-Fi card');
		if (from !== null) leaveWifi(from);
		const { ssid, passphrase } = networkNamed(real, network);
		await joinWifi(real.hubsDevice, ssid, passphrase);
		real.hubsNetwork = network;
		for (const hub of m.hubs) m.hubJoin(hub.name, network);
		await checkHubsOn(m, real, network, `the hubs joined ${network}`);
		if (from !== null) {
			await checkHubsOn(m, real, from, `the hubs left ${from}`);
		}
	}

	toString(): string {
		return `hubJoin(${this.networkIdx})`;
	}
}

/** Take the hubs off the air — the host's card leaving its LAN. Phones
 *  there must drop them. */
class HubLeaveMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.hubs.some(h => h.network !== null);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const from = real.hubsNetwork;
		if (from === null) throw new Error('the hubs are on no network');
		log(`${this.toString()} -> hubs leave ${from}`);
		leaveWifi(from);
		real.hubsNetwork = null;
		for (const hub of m.hubs) m.hubLeave(hub.name);
		await checkHubsOn(m, real, from, `the hubs left ${from}`);
	}

	toString(): string {
		return 'hubLeave()';
	}
}

export const hubMoves: Moves = [
	{ arbitrary: fc.constant(new CreateHubMove()), weight: 2 },
	{ arbitrary: fc.nat().map(h => new StartHubMove(h)), weight: 4 },
	{ arbitrary: fc.nat().map(h => new StopHubMove(h, 'SIGINT')), weight: 2 },
	{ arbitrary: fc.nat().map(h => new StopHubMove(h, 'SIGKILL')), weight: 1 },
	{ arbitrary: fc.nat().map(n => new HubJoinMove(n)), weight: 4 },
	{ arbitrary: fc.constant(new HubLeaveMove()), weight: 2 },
];

/** The hub moves by the names a search prints them under. */
export const move = {
	createHub: (): Move => new CreateHubMove(),
	startHub: (hubIdx: number): Move => new StartHubMove(hubIdx),
	stopHub: (hubIdx: number): Move => new StopHubMove(hubIdx, 'SIGINT'),
	killHub: (hubIdx: number): Move => new StopHubMove(hubIdx, 'SIGKILL'),
	hubJoin: (networkIdx: number): Move => new HubJoinMove(networkIdx),
	hubLeave: (): Move => new HubLeaveMove(),
};
