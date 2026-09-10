/**
 * A phone with the app open walks into a network where a hub has been running
 * all along. The hub never announces again, so the app has to find it by
 * re-querying when the phone's address changes — and the hub has to be usable
 * within a couple of seconds of joining, which is what a user expects.
 *
 * The hub's network is a hotspot the host raises for the run, with the hub — a
 * standalone `mailbox-local-server`, the binary the docker image ships — bound
 * to the host's address on it. From the phone's usual Wi-Fi the hub is
 * announced but unreachable, exactly like a hub on another LAN. The phone joins
 * the hotspot and, to leave, forgets it and falls back to its usual network.
 * The cloud mailbox is killed before the app launches, so the connection chip
 * is on screen throughout.
 *
 * Skips itself unless E2E_STRESS=1: it needs a physical phone, and it takes
 * over the host's Wi-Fi card while it runs.
 *   PLATFORMS=android E2E_STRESS=1 just e2e run local-hub-network-switch
 */
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { UI_TIMEOUT } from '../helpers/timeouts';
import {
	type Hotspot,
	startHotspot,
	stopHotspot,
	wifiDevices,
} from '../setup/hotspot';
import { type LocalHub, spawnLocalHub, stopLocalHub } from '../setup/local-hub';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** From the phone holding an address on the hub's network to the chip reading
 *  local. */
const DISCOVERY_MS = 2_000;
/** Time back on the usual network before the next join, so the app has fully
 *  given the hub up. */
const AWAY_MS = 30_000;
/** Joins measured: one can get lucky. */
const JOINS = 3;

// wdio arms its per-test abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside the test body.
describe('Local hub discovery after a network switch', function () {
	this.timeout(JOINS * (AWAY_MS + 3 * UI_TIMEOUT) + 300_000);

	let phone: Agent;
	let hotspot: Hotspot;
	let hub: LocalHub;
	let mailboxKilled = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (isRemoteMailbox()) this.skip();
		await killMailbox();
		mailboxKilled = true;

		hotspot = await startHotspot('dash-e2e-hotspot', wifiDevices()[0]);
		hub = await spawnLocalHub('a', hotspot.address);
		[phone] = await setupAgents(this, [{ platform: 'android' }]);

		if (phone.platform === 'android-emulator') this.skip();

		await phone.createProfilePage.createProfile('Alice', 'Hub');

		await createGroup(phone, 'Solo Group', []);
		// On its usual network the phone hears the hub announced but cannot
		// reach it, so the chip must not claim a hub yet.
		await phone.groupChatPage.connectionStatusIndicator.waitForStatus(
			'disconnected',
			UI_TIMEOUT,
			"the chip showed a hub before the phone joined the hub's network, so " +
				"the hub is reachable from the phone's usual LAN and the switch would " +
				'prove nothing',
		);
	});

	after(async () => {
		if (phone !== undefined && hotspot !== undefined) {
			try {
				await phone.forgetWifi(hotspot.ssid);
			} catch {
				/* the phone is back on its usual network either way */
			}
		}
		if (hub !== undefined) await stopLocalHub(hub);
		if (hotspot !== undefined) stopHotspot(hotspot.ssid);
		if (mailboxKilled) await restartMailbox();
	});

	it("shows the hub within 2 seconds of joining the hub's network", async () => {
		const chip = phone.groupChatPage.connectionStatusIndicator;
		for (let join = 1; join <= JOINS; join++) {
			await phone.connectWifi(hotspot.ssid, hotspot.passphrase);
			const joinedAt = Date.now();
			await chip.waitForStatus(
				'local',
				DISCOVERY_MS,
				`join ${join}: the hub was still not shown ${DISCOVERY_MS / 1_000}s ` +
					'after the phone was on its network, with the app on screen the ' +
					'whole time',
			);
			// Host time up front, so the line lines up with the phone and hub logs.
			console.log(
				`${new Date().toISOString().slice(11, 23)} join ${join}: hub shown ${Date.now() - joinedAt}ms after joining`,
			);
			await phone.forgetWifi(hotspot.ssid);
			await chip.waitForStatus(
				'disconnected',
				UI_TIMEOUT,
				`join ${join}: the chip still showed the hub after the phone left its network`,
			);
			await phone.pause(AWAY_MS);
		}
	});
});
