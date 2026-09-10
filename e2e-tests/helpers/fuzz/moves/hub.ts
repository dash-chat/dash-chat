/** Moves a local hub makes: coming into being, and joining or leaving a LAN.
 *  Each ends by asserting what the connection chips show, so a sequence
 *  fails at the exact move hub discovery did not survive. */
import fc from 'fast-check';

import { allocateFreePort } from '../../../setup/allocate-port';
import { spawnLocalHub } from '../../../setup/local-hub';
import { type Real, at, hubNamed, log, networkNamed, parkHub } from '../agents';
import { checkHubsOn } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Hubs a run may create: each is a process on the host. */
const MAX_HUBS = 2;

/** Give a hub an identity — db, key, port — without putting it anywhere. */
class CreateHubMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.networkCapacity > 0 && m.hubs.length < MAX_HUBS;
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

/** Put a hub on a LAN it is not on — bringing it up there, or moving it
 *  from where it was. Phones on that LAN must show it; phones on the LAN it
 *  left must drop it. */
class HubJoinMove extends Move {
	constructor(
		readonly hubIdx: number,
		readonly networkIdx: number,
	) {
		super();
	}

	private movable(m: Readonly<ExpectedModel>) {
		return m.hubs.filter(h => m.networkNames().some(n => n !== h.network));
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.movable(m).length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
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
		m.hubJoin(hub.name, network);
		await checkHubsOn(m, real, network, `${hub.name} joined ${network}`);
		if (from !== null) {
			await checkHubsOn(m, real, from, `${hub.name} left ${from}`);
		}
	}

	toString(): string {
		return `hubJoin(${this.hubIdx},${this.networkIdx})`;
	}
}

class HubLeaveMove extends Move {
	constructor(readonly hubIdx: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.hubs.some(h => h.network !== null);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const hub = at(
			m.hubs.filter(h => h.network !== null),
			this.hubIdx,
		);
		const from = hub.network;
		if (from === null) throw new Error(`${hub.name} is on no network`);
		log(`${this.toString()} -> ${hub.name} leaves ${from}`);
		await parkHub(hubNamed(real, hub.name));
		m.hubLeave(hub.name);
		await checkHubsOn(m, real, from, `${hub.name} left ${from}`);
	}

	toString(): string {
		return `hubLeave(${this.hubIdx})`;
	}
}

export const hubMoves: Moves = [
	{ arbitrary: fc.constant(new CreateHubMove()), weight: 2 },
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([h, n]) => new HubJoinMove(h, n)),
		weight: 4,
	},
	{ arbitrary: fc.nat().map(h => new HubLeaveMove(h)), weight: 2 },
];

/** The hub moves by the names a search prints them under. */
export const move = {
	createHub: (): Move => new CreateHubMove(),
	hubJoin: (hubIdx: number, networkIdx: number): Move =>
		new HubJoinMove(hubIdx, networkIdx),
	hubLeave: (hubIdx: number): Move => new HubLeaveMove(hubIdx),
};
