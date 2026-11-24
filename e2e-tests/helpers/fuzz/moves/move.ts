import fc from 'fast-check';
import path from 'node:path';

import {
	failureSlug,
	failuresDir,
	saveFailureScreenshot,
} from '../../../setup/failure-screenshots';
import { type Real, type StressAgent, at, byName, log } from '../agents';
import { expectNotifications, settle } from '../checks';
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

	/** A failure is logged here as well as thrown, with every agent's screen
	 *  saved as it was: the search only reports it once shrinking is done,
	 *  which can be many replays later. */
	async run(m: ExpectedModel, real: Real): Promise<void> {
		try {
			await this.perform(m, real);
			await settle(m, real);
			await expectNotifications(m, real);
		} catch (err) {
			log(
				`${this.toString()} failed: ${err instanceof Error ? (err.stack ?? err.message) : String(err)}`,
			);
			await saveScreens(real, this.toString());
			throw err;
		}
	}
}

async function saveScreens(real: Real, move: string): Promise<void> {
	const stamp = new Date().toISOString().slice(11, 19).replace(/:/g, '-');
	for (const sa of real.agents) {
		await saveFailureScreenshot(
			sa.agent,
			path.join(failuresDir(), `${stamp}-${failureSlug(move)}-${sa.name}.png`),
		);
	}
}

/** A kind of move a pool offers: built from abstract indices, and drawn
 *  `weight` times as often as one of weight 1. */
export interface MoveKind {
	build(a: number, b: number, c: number): Move;
	weight: number;
}

/** A pool of move kinds with their weights; pools are spread together. */
export type Moves = readonly MoveKind[];

/** The most an abstract index counts to. Indices resolve modulo the options
 *  there are, never more than a handful, and a small range keeps shrinking
 *  one to a few replays rather than thirty. */
const INDEX_MAX = 1023;

/** One step of a sequence: whichever kind of move can be made when it is
 *  reached, so a sequence of n steps is n moves rather than the few whose
 *  preconditions a blind draw happened to meet. `kindIdx` picks among the
 *  applicable kinds, each repeated by its weight; the other indices are the
 *  move's own. */
export class Step implements fc.AsyncCommand<ExpectedModel, Real> {
	private resolved: Move | null = null;

	constructor(
		private readonly kinds: Moves,
		private readonly kindIdx: number,
		private readonly a: number,
		private readonly b: number,
		private readonly c: number,
	) {}

	private applicable(m: Readonly<ExpectedModel>): MoveKind[] {
		return this.kinds.filter(k => k.build(this.a, this.b, this.c).check(m));
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return this.applicable(m).length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const weighted = this.applicable(m).flatMap(k =>
			Array<MoveKind>(k.weight).fill(k),
		);
		this.resolved = at(weighted, this.kindIdx).build(this.a, this.b, this.c);
		await this.resolved.run(m, real);
	}

	/** The move it resolved to, once run; a search prints that as the
	 *  reproduction, since it is what actually happened. */
	toString(): string {
		if (this.resolved !== null) return this.resolved.toString();
		return `step(${this.kindIdx},${this.a},${this.b},${this.c})`;
	}
}

function index(): fc.Arbitrary<number> {
	return fc.nat({ max: INDEX_MAX });
}

/** Steps over `kinds`, one per draw. */
export function steps(kinds: Moves): fc.Arbitrary<Step> {
	if (kinds.length === 0) throw new Error('no moves to draw from');
	return fc
		.tuple(index(), index(), index(), index())
		.map(([k, a, b, c]) => new Step(kinds, k, a, b, c));
}

/**
 * A move one agent makes. It carries an abstract `agentIdx` rather than a
 * name: `actors` says who could make it in the state it finds, and the index
 * resolves against them, so a drawn sequence stays valid however the world
 * changed and shrinking can lower indices freely. Saying who can act is also
 * what makes the move applicable at all, so `check` follows from it.
 */
export abstract class ActorMove extends Move {
	constructor(readonly agentIdx: number) {
		super();
	}

	/** The agents that can make this move right now. */
	abstract actors(m: Readonly<ExpectedModel>): string[];

	check(m: Readonly<ExpectedModel>): boolean {
		return this.actors(m).length > 0;
	}

	/** The one `agentIdx` picks among them. */
	protected actor(m: Readonly<ExpectedModel>, real: Real): StressAgent {
		return byName(real, at(this.actors(m), this.agentIdx));
	}
}
