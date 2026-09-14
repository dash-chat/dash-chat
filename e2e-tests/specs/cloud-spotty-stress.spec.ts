/**
 * Long-lived spotty-cloud stress: two agents behave like normal users —
 * adding each other as contacts, creating groups, sending texts and photos,
 * reacting, replying, editing, deleting, backgrounding and restarting the
 * app — while the link to the cloud mailbox turns slow, hangs, refuses
 * connections and heals under them. After every move, each agent that
 * should have received something is checked for exactly it, and every
 * cloud move checks what each agent's connection chip says of the cloud.
 *
 * Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=android,android just e2e run cloud-spotty-stress
 *
 * Tunables: E2E_STRESS_COMMANDS (default 80, roughly several minutes),
 * E2E_STRESS_SEED (default random; the run logs it — re-run with the same
 * seed to reproduce a failure).
 */
import { Fuzzer } from '../helpers/fuzz/fuzzer';
import { cloudMoves } from '../helpers/fuzz/moves/cloud';
import { deviceMoves } from '../helpers/fuzz/moves/device';
import { userMoves } from '../helpers/fuzz/moves/user';
import { envInt } from '../helpers/utils';
import { isRemoteMailbox, mailboxLink } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Spotty cloud stress', () => {
	let agent1: Agent;
	let agent2: Agent;
	let fuzzer: Fuzzer;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The mailbox's link must be ours to degrade, which a remote
		// environment's is not.
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Stress');
		await agent2.createProfilePage.createProfile('Bob', 'Stress');
		fuzzer = await Fuzzer.prepare(this, {
			agents: [
				{ agent: agent1, name: 'Alice' },
				{ agent: agent2, name: 'Bob' },
			],
			cloud: mailboxLink(),
		});
	});

	it('agents behave normally for the whole run while the cloud link flaps', async () => {
		const commands = envInt('E2E_STRESS_COMMANDS', 80);
		const seed = envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31));
		await fuzzer.soak({
			moves: [...userMoves, ...deviceMoves, ...cloudMoves],
			length: commands,
			seed,
		});
	});
});
