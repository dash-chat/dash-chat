/**
 * A hub already running when its host *gains* a network must re-announce on it —
 * otherwise a phone that joins that network never discovers the hub. This is the
 * mirror of local-hub-network-switch: there the hub stays put and the phone
 * moves; here the phone stays on one network and the hub's host gains it.
 *
 * Skips itself unless E2E_STRESS=1: it needs a physical phone and takes over the
 * host's Wi-Fi card while it runs.
 *   PLATFORMS=android E2E_STRESS=1 just e2e run local-hub-announce-rearm
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

/** From joining the hotspot to the chip reading local. Generous on purpose: the
 *  point is that the hub appears at all (vs. never, without the re-arm), not the
 *  two-second bar. */
const DISCOVERY_MS = 5_000;

describe('Local hub re-announces when its host gains a network', function () {
	this.timeout(600_000);

	let phone: Agent;
	let hotspot: Hotspot;
	let hub: LocalHub;
	let mailboxKilled = false;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (isRemoteMailbox()) this.skip();
		await killMailbox();
		mailboxKilled = true;

		[phone] = await setupAgents(this, [{ platform: 'android' }]);
		// An emulator is NAT'd off the runner's LAN and cannot change networks.
		if (phone.platform === 'android-emulator') this.skip();
		await phone.createProfilePage.createProfile('Alice', 'Hub');
		await createGroup(phone, 'Solo Group', []);

		await phone.disableWifi();

		hub = await spawnLocalHub('rearm');
		await phone.groupChatPage.connectionStatusIndicator.waitForStatus(
			'disconnected',
			UI_TIMEOUT,
			'the phone reached the hub before joining its network — the hub must ' +
				'be isolated from the phone for this test to prove anything',
		);

		hotspot = await startHotspot('dash-e2e-hotspot', wifiDevices()[0]);
	});

	after(async () => {
		if (phone !== undefined) {
			try {
				if (hotspot !== undefined) await phone.forgetWifi(hotspot.ssid);
				await phone.enableWifi();
			} catch {
				/* the phone is back on its usual network either way */
			}
		}
		if (hub !== undefined) await stopLocalHub(hub);
		if (hotspot !== undefined) stopHotspot(hotspot.ssid);
		if (mailboxKilled) await restartMailbox();
	});

	it('shows a hub that re-announced on a network its host gained after startup', async () => {
		await phone.connectWifi(hotspot.ssid, hotspot.passphrase);
		await phone.groupChatPage.connectionStatusIndicator.waitForStatus(
			'local',
			DISCOVERY_MS,
			"the hub was never shown after joining the host's newly-raised hotspot — " +
				'the announcer is not re-arming on the host network change, so a hub ' +
				'that gains an interface stays undiscoverable until it restarts',
		);
	});
});
