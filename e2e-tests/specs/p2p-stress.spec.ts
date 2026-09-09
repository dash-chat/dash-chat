/**
 * Long-lived offline stress: two agents behave like normal users — adding
 * each other as contacts, creating groups, sending texts and photos,
 * reacting, replying, editing, deleting, backgrounding and restarting the
 * app — while the cloud mailbox is suspended, so every operation must
 * propagate over a direct p2p (iroh/mDNS) connection. Convergence is
 * verified at in-pool checkpoints and at the end of the run.
 *
 * Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=android,android just e2e run p2p-stress
 *
 * Tunables: E2E_STRESS_COMMANDS (default 80, roughly several minutes),
 * E2E_STRESS_SEED (default random; the run logs it — re-run with the same
 * seed to reproduce a failure).
 */
import fc from 'fast-check';

import { userCommand } from '../helpers/fast-check/commands';
import { run, setupFastCheck } from '../helpers/fast-check/system';
import { envInt } from '../helpers/utils';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('P2P offline stress', () => {
	let agent1: Agent;
	let agent2: Agent;
	let mailboxSuspended = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The mailbox must be suspendable, which a remote environment's is not.
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		// Down before anything syncs, so contact exchange and everything after
		// it must travel over a direct p2p connection.
		suspendMailbox();
		mailboxSuspended = true;
		await agent1.createProfilePage.createProfile('Alice', 'Stress');
		await agent2.createProfilePage.createProfile('Bob', 'Stress');
	});

	after(() => {
		if (!mailboxSuspended) return;
		try {
			resumeMailbox();
		} catch {
			/* mailbox process already gone */
		}
	});

	it('agents behave normally for the whole run over p2p sync only', async function () {
		const commands = envInt('E2E_STRESS_COMMANDS', 80);
		const seed = envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31));
		// Commands drive real UI flows and checkpoints wait out generous p2p
		// sync timeouts, so budget well beyond the expected pace.
		this.timeout(commands * 30_000 + 300_000);
		const system = await setupFastCheck({
			agents: [
				{ agent: agent1, name: 'Alice' },
				{ agent: agent2, name: 'Bob' },
			],
		});
		await run(
			system,
			// A fixed-length array rather than fc.commands: its length is
			// exactly the run's size, where fc.commands draws a random length
			// that can come out near-empty even with size: 'max'.
			fc.array(userCommand, { minLength: commands, maxLength: commands }),
			// One execution against real devices; a failure reports the seed and
			// command sequence instead of shrinking (each replay would cost a
			// full multi-minute run against freshly reset devices). Unbiased
			// because the bias shrinks early runs' sequences toward empty — and
			// with a single run there is only an "early" run.
			{ numRuns: 1, seed, endOnFailure: true, unbiased: true },
		);
	});
});
