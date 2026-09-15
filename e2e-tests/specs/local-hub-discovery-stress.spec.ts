/**
 * Hub discovery under random network life: local hubs start, stop, die and
 * move between real Wi-Fi networks while phones walk in and out of them,
 * background, restart and sit still — with the cloud mailbox killed, so
 * the connection chip is what says which hubs a phone sees. After every
 * move, each phone that should have noticed is checked for exactly the hubs
 * on its LAN.
 *
 * Skips itself unless E2E_STRESS=1 and E2E_WIFI_NETWORKS names at least one
 * network (see .env.development.example). Run it with:
 *   PLATFORMS=android,android just e2e run local-hub-discovery-stress
 *
 * Tunables: E2E_STRESS_ATTEMPTS (sequences to try, default 20),
 * E2E_STRESS_COMMANDS (moves per sequence, default 15), E2E_STRESS_SEED
 * (default random; the run logs it — re-run with the same seed to reproduce
 * a failure).
 */
import { Fuzzer } from '../helpers/fuzz/fuzzer';
import { deviceMoves } from '../helpers/fuzz/moves/device';
import { hubMoves } from '../helpers/fuzz/moves/hub';
import { networkMoves } from '../helpers/fuzz/moves/network';
import { envInt } from '../helpers/utils';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { wifiNetworks } from '../setup/test-env';

/** Preparation walks two phones through Settings several times and pairs
 *  them up, which runs past mocha's 5-minute default. */
const PREPARE_TIMEOUT_MS = 20 * 60 * 1_000;

// wdio arms its per-hook abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside the hook body; the tests
// themselves get theirs from Fuzzer.prepare.
describe('Local hub stress', function () {
	this.timeout(PREPARE_TIMEOUT_MS);

	let agent1: Agent;
	let agent2: Agent;
	let fuzzer: Fuzzer;
	let mailboxKilled = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (wifiNetworks().length === 0) this.skip();
		// The mailbox must be killable, which a remote environment's is not.
		if (isRemoteMailbox()) this.skip();
		// Only a physical phone can change network without losing its driver
		// session.
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'mobile' },
		]);
		// An emulator is NAT'd off the host, so no test network can reach it.
		if ([agent1, agent2].some(a => a.platform === 'android-emulator')) {
			this.skip();
		}
		// Down before anything syncs, and for the chip to be on screen at all.
		// Killed, not suspended: a network change wakes every mailbox poller,
		// and a suspended cloud then counts as connected until its polls time
		// out again, hiding the chip for any hub found meanwhile.
		await killMailbox();
		mailboxKilled = true;
		await agent1.createProfilePage.createProfile('Alice', 'Stress');
		await agent2.createProfilePage.createProfile('Bob', 'Stress');
		fuzzer = await Fuzzer.prepare(this, {
			agents: [
				{ agent: agent1, name: 'Alice' },
				{ agent: agent2, name: 'Bob' },
			],
			networks: wifiNetworks(),
		});
	});

	after(async () => {
		if (mailboxKilled) await restartMailbox();
	});

	it('phones show exactly the hubs on their LAN through every move', async () => {
		const attempts = envInt('E2E_STRESS_ATTEMPTS', 20);
		const length = envInt('E2E_STRESS_COMMANDS', 15);
		const seed = envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31));
		await fuzzer.search({
			moves: [...hubMoves, ...networkMoves, ...deviceMoves],
			attempts,
			length,
			seed,
		});
	});
});
