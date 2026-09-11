/** Moves a device makes on its own: leaving and returning to the foreground,
 *  and restarting the app. */
import fc from 'fast-check';

import { type Real, at, byName, log } from '../agents';
import { checkHubs } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Backgrounds an agent and leaves it backgrounded: later moves keep acting
 * through the other agents (including sending to this one), and a
 * ForegroundMove brings it back, making catch-up-after-background part of
 * every run. */
class BackgroundMove extends Move {
	constructor(readonly agentIdx: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeMobileNames().length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.activeMobileNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.backgroundApp();
		m.background(actor.name);
	}

	toString(): string {
		return `background(${this.agentIdx})`;
	}
}

class ForegroundMove extends Move {
	constructor(readonly agentIdx: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.backgroundedNames().length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.backgroundedNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.startApp();
		await actor.agent.homePage.ready();
		m.foreground(actor.name);
		if (m.hasNetworks()) {
			await checkHubs(m, actor, 'coming back to the foreground');
		}
	}

	toString(): string {
		return `foreground(${this.agentIdx})`;
	}
}

class RestartMove extends Move {
	constructor(readonly agentIdx: number) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().length > 0;
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
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
		if (m.hasNetworks()) {
			await checkHubs(m, actor, 'the app restarted');
		}
	}

	toString(): string {
		return `restart(${this.agentIdx})`;
	}
}

export const deviceMoves: Moves = [
	{ arbitrary: fc.nat().map(a => new BackgroundMove(a)), weight: 3 },
	{ arbitrary: fc.nat().map(a => new ForegroundMove(a)), weight: 3 },
	{ arbitrary: fc.nat().map(a => new RestartMove(a)), weight: 1 },
];

/** The device moves by the names a search prints them under. */
export const move = {
	background: (agentIdx: number): Move => new BackgroundMove(agentIdx),
	foreground: (agentIdx: number): Move => new ForegroundMove(agentIdx),
	restart: (agentIdx: number): Move => new RestartMove(agentIdx),
};
