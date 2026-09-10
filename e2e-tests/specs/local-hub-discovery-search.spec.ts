/**
 * For any network history, the app shows the hubs it can reach within two
 * seconds. The fuzzer draws sequences of LANs being raised and killed, hubs
 * being created and moved between them, phones walking in and out, apps going
 * to the background and back or restarting, and minutes spent sitting still;
 * every move asserts what the connection chip must show. A failing sequence is
 * shrunk to the shortest one that still fails and printed as a reproduction to
 * paste into a `replay`.
 *
 * Skips itself unless E2E_STRESS=1: it needs two physical phones, and takes
 * over the host's Wi-Fi cards while it runs — one LAN can be up per card.
 *   PLATFORMS=android,android E2E_STRESS=1 just e2e run local-hub-discovery-search
 *
 * Tunables: E2E_STRESS_RUNS (sequences to try, default 10), E2E_STRESS_COMMANDS
 * (moves per sequence, default 15), E2E_STRESS_SEED (default random; the run
 * logs it and a failure reports it — re-run with the same seed to reproduce).
 */
import { Fuzzer } from '../helpers/fuzz/fuzzer';
import { deviceMoves } from '../helpers/fuzz/moves/device';
import { hubMoves } from '../helpers/fuzz/moves/hub';
import { networkMoves } from '../helpers/fuzz/moves/network';
import { envInt } from '../helpers/utils';
import { wifiDevices } from '../setup/hotspot';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Local hub discovery', function () {
	let alice: Agent;
	let bob: Agent;
	let fuzzer: Fuzzer;
	let mailboxKilled = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The suite kills the cloud mailbox server's process, which is
		// impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		await killMailbox();
		mailboxKilled = true;
		[alice, bob] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'android' },
		]);
		// An emulator is NAT'd off the runner's LAN and cannot change networks.
		if (
			alice.platform === 'android-emulator' ||
			bob.platform === 'android-emulator'
		) {
			this.skip();
		}
		await alice.createProfilePage.createProfile('Alice', 'Hub');
		await bob.createProfilePage.createProfile('Bob', 'Hub');
		fuzzer = await Fuzzer.prepare({
			agents: [
				{ agent: alice, name: 'Alice' },
				{ agent: bob, name: 'Bob' },
			],
			wifiDevices: wifiDevices(),
		});
	});

	after(async () => {
		if (mailboxKilled) await restartMailbox();
	});

	it('shows the reachable hubs and reaches the reachable peers after any move', async function () {
		await fuzzer.search(this, {
			moves: [...networkMoves, ...hubMoves, ...deviceMoves],
			attempts: envInt('E2E_STRESS_RUNS', 10),
			length: envInt('E2E_STRESS_COMMANDS', 15),
			seed: envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31)),
		});
	});
});
