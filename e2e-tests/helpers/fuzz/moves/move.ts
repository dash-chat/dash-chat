import type fc from 'fast-check';

import type { Real } from '../agents';
import { settle } from '../checks';
import type { ExpectedModel } from '../model';

/**
 * One thing a user, a device, a hub or a network does. `perform` drives it
 * through the real system and records its ops in the model; `run` then
 * settles, so every agent the model says learnt something is checked before
 * the next move. Moves carry abstract indices, resolved modulo the eligible
 * options at run time, so any generated sequence is valid.
 */
export abstract class Move implements fc.AsyncCommand<ExpectedModel, Real> {
	abstract check(m: Readonly<ExpectedModel>): boolean;
	abstract perform(m: ExpectedModel, real: Real): Promise<void>;
	abstract toString(): string;

	async run(m: ExpectedModel, real: Real): Promise<void> {
		await this.perform(m, real);
		await settle(m, real);
	}
}

export interface WeightedMove {
	arbitrary: fc.Arbitrary<Move>;
	weight: number;
}

/** A pool of moves with their weights; pools are spread together. */
export type Moves = readonly WeightedMove[];
