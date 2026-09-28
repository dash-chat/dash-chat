/**
 * Long-lived spotty-cloud stress: two agents behave like normal users —
 * adding each other as contacts, creating groups, sending texts and photos,
 * reacting, replying, editing, deleting, backgrounding and restarting the
 * app — while the link to the cloud mailbox turns slow, hangs, refuses
 * connections and heals under them. After every move, each agent that
 * should have received something is checked for exactly it, and every
 * cloud move checks what each agent's connection chip says of the cloud.
 * A failing sequence is shrunk to the smallest that still fails.
 *
 * Both agents run without p2p, so the cloud link is the only way an op can
 * travel. Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=desktop,desktop just e2e run cloud-spotty-stress
 *
 * Tunables: E2E_STRESS_ATTEMPTS (sequences to try, default 20),
 * E2E_STRESS_COMMANDS (moves per sequence, default 40), E2E_STRESS_SEED
 * (default random; the run logs it — re-run with the same seed to reproduce
 * a failure).
 */
import { Fuzzer } from '../helpers/fuzz/fuzzer';
import { blockMoves } from '../helpers/fuzz/moves/block';
import { cloudMoves } from '../helpers/fuzz/moves/cloud';
import { exchangeContactMoves } from '../helpers/fuzz/moves/contacts';
import { deviceMoves } from '../helpers/fuzz/moves/device';
import { groupMoves } from '../helpers/fuzz/moves/groups';
import { mediaMoves } from '../helpers/fuzz/moves/media';
import { profileMoves } from '../helpers/fuzz/moves/profile';
import { textMessageMoves } from '../helpers/fuzz/moves/text-messages';
import { envInt } from '../helpers/utils';
import { isRemoteMailbox } from '../setup/mailbox-control';
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
		// Without p2p every op has to travel through the cloud mailbox, so
		// its link is the only thing the checks measure.
		await Promise.all([agent1.disableP2p(), agent2.disableP2p()]);
		const agents = { Alice: agent1, Bob: agent2 };
		fuzzer = await Fuzzer.prepare(this, { agents });
	});

	it('agents behave normally for the whole run while the cloud link flaps', async () => {
		const attempts = envInt('E2E_STRESS_ATTEMPTS', 20);
		const length = envInt('E2E_STRESS_COMMANDS', 40);
		const seed = envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31));
		await fuzzer.search({
			moves: [
				...exchangeContactMoves,
				...blockMoves,
				...textMessageMoves,
				...mediaMoves,
				...groupMoves,
				...profileMoves,
				...deviceMoves,
				...cloudMoves,
			],
			attempts,
			length,
			seed,
		});
	});
});
