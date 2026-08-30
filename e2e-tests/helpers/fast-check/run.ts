/**
 * Random "normal user" behavior for stress runs, built on fast-check's
 * model-based testing. fast-check owns the randomness: a failing run reports
 * its seed and the exact command sequence, and re-running with
 * `E2E_STRESS_SEED` reproduces it.
 *
 * Convergence is checked by an in-pool checkpoint command and once more at
 * the end of the run: every agent must render every chat and each expected
 * message's current state (presence, edited text, deletion, reactions).
 *
 * Connectivity is the caller's business: a spec sets up the scenario it
 * wants (mailbox suspended for pure p2p, local mailbox only, everything on)
 * and then hands its agents to `runRandomBehavior`.
 */
import fc from 'fast-check';

import type { Agent } from '../../setup/setup-agents';
import type { AgentPlatformName } from '../../setup/test-env';
import { navigateToAddContact } from '../flows/exchange-contacts';
import { type Real, log } from './agents';
import { commandArbitrary } from './commands';
import { ExpectedModel } from './model';
import { verifyConvergence } from './verify';

export interface BehaviorAgent {
	agent: Agent;
	name: string;
	/** Gates platform-specific commands: background/foreground run only
	 * against mobile agents. */
	platform: AgentPlatformName;
}

export interface RandomBehaviorOptions {
	/** How many commands to generate — the run's length. */
	commands: number;
	seed: number;
}

/**
 * Bootstrap the agents (preview features + contact links), then run one
 * seeded random command sequence against them, verifying convergence at the
 * in-pool checkpoints and once more at the end. Reproduce a failure by
 * re-running with the seed fast-check reports.
 */
export async function runRandomBehavior(
	agents: BehaviorAgent[],
	opts: RandomBehaviorOptions,
): Promise<void> {
	log(`seed=${opts.seed} commands=${opts.commands}`);
	const real: Real = { agents: [] };
	for (const { agent, name } of agents) {
		await agent.enablePreviewFeatures();
		await agent.homePage.ready();
		await navigateToAddContact(agent);
		const link = await agent.addContactPage.getAddContactLink();
		await agent.addContactPage.back.click();
		await agent.newMessagePage.back.click();
		await agent.homePage.ready();
		real.agents.push({ agent, name, link });
	}

	let model!: ExpectedModel;
	await fc.assert(
		fc.asyncProperty(
			// A fixed-length array rather than fc.commands: its length is exactly
			// the run's size, where fc.commands draws a random length that can
			// come out near-empty even with size: 'max'.
			fc.array(commandArbitrary, {
				minLength: opts.commands,
				maxLength: opts.commands,
			}),
			async sequence => {
				log(`generated ${sequence.length} command(s)`);
				model = new ExpectedModel(
					agents.map(({ name, platform }) => ({
						name,
						mobile: platform !== 'desktop',
					})),
				);
				await fc.asyncModelRun(() => ({ model, real }), sequence);
				await verifyConvergence(model, real);
			},
		),
		// One execution against real devices; a failure reports the seed and
		// command sequence instead of shrinking (each replay would cost a full
		// multi-minute run against freshly reset devices). Unbiased because the
		// bias shrinks early runs' sequences toward empty — and with a single
		// run there is only an "early" run.
		{ numRuns: 1, seed: opts.seed, endOnFailure: true, unbiased: true },
	);
	const messages = model.chats.reduce((n, c) => n + c.messages.length, 0);
	log(`done: ${model.chats.length} chat(s), ${messages} message(s)`);
}
