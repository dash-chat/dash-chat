/** Sitting still: the one move that spends wall-clock rather than doing
 *  something, for what only shows after time passes — a discovery record
 *  ageing out of a cache, a poller's next tick. Nothing draws it by default;
 *  a spec whose property needs the waiting adds it to its own pool with the
 *  range it cares about. */
import { type Real, byName, log } from '../agents';
import { checkConnection } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type MoveKind } from './move';

/** Sits still with the connection chip on screen, then every agent's chip
 * must still read what the model says: the hubs on its LAN, or the cloud
 * link's state. */
class SleepMove extends Move {
	constructor(readonly seconds: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		const watching = m.activeNames().map(name => byName(real, name));
		for (const sa of watching)
			await checkConnection(m, sa, 'before sitting still');
		await watching[0].agent.pause(this.seconds * 1_000);
		for (const sa of watching) {
			await checkConnection(m, sa, `sitting still for ${this.seconds}s`);
		}
	}

	toString(): string {
		return `sleep(${this.seconds})`;
	}
}

/** Sitting still for a drawn number of seconds between `min` and `max`. */
export function sleepMove(min: number, max: number): MoveKind {
	return {
		build: s => new SleepMove(min + (s % (max - min + 1))),
		weight: 1,
	};
}

/** The sleep move by the name a search prints it under. */
export const move = {
	sleep: (seconds: number): Move => new SleepMove(seconds),
};
