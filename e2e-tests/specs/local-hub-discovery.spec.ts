/**
 * Local hub discovery on the paths it is known to break on, walked in order on
 * a real phone and a real access point: a hub the phone joins its LAN to find,
 * the hub stopping and starting under a foregrounded and a backgrounded app,
 * the phone and then the hub's host leaving and rejoining the LAN, a sit-still
 * past the mDNS record TTL, and a Wi-Fi bounce. The hub is the standalone
 * `mailbox-local-server` on the host, on the LAN through the host's Wi-Fi
 * card; the cloud mailbox is killed so the chip is on screen and reads the
 * hub. local-hub-discovery-stress walks the same moves at random.
 *
 * Skips itself unless E2E_STRESS=1 and E2E_WIFI_NETWORKS names a network (see
 * .env.development.example). Run it with:
 *   PLATFORMS=android just e2e run local-hub-discovery
 */
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { DISCOVERY_MS } from '../helpers/fuzz/checks';
import { MDNS_RECORD_TTL_S } from '../helpers/fuzz/moves/network';
import { UI_TIMEOUT } from '../helpers/timeouts';
import { joinWifi, leaveWifi, wifiDevice } from '../setup/host-wifi';
import {
	type LocalHub,
	restartLocalHub,
	spawnLocalHub,
	stopLocalHub,
} from '../setup/local-hub';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { wifiNetworks } from '../setup/test-env';

/** Time for the OS to stop delivering multicast to the backgrounded app. */
const BACKGROUNDED_MS = 60_000;
/** Hub restarts in a row: one only ever samples a single announcement. */
const RESTARTS = 3;
/** Wi-Fi down long enough that the supplicant tears the association down
 *  rather than treating it as a blip. */
const WIFI_DOWN_MS = 8_000;

// wdio arms its per-test abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside the test body.
describe('Local hub discovery', function () {
	this.timeout(600_000);

	const network = wifiNetworks()[0];
	let hostDevice: string;
	let phone: Agent;
	let hub: LocalHub;
	let mailboxKilled = false;

	const chip = () => phone.groupChatPage.connectionStatusIndicator;

	async function expectLocal(after: string): Promise<void> {
		await chip().waitForStatus(
			'local',
			DISCOVERY_MS,
			`the chip did not read local within ${DISCOVERY_MS / 1_000}s after ${after}`,
		);
	}

	async function expectNoHub(after: string): Promise<void> {
		await chip().waitForNotLocal(
			DISCOVERY_MS,
			`the chip still showed the hub ${DISCOVERY_MS / 1_000}s after ${after}`,
		);
	}

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (network === undefined) this.skip();
		if (isRemoteMailbox()) this.skip();
		const device = wifiDevice();
		if (device === null) throw new Error('the host has no Wi-Fi card');
		hostDevice = device;
		await killMailbox();
		mailboxKilled = true;

		[phone] = await setupAgents(this, [{ platform: 'android' }]);
		// An emulator is NAT'd off the host and cannot change networks.
		if (phone.platform === 'android-emulator') this.skip();
		await phone.createProfilePage.createProfile('Alice', 'Hub');
		await createGroup(phone, 'Solo Group', []);
		// Off the air: the hub answers on the host's usual LAN too, and the phone
		// must not find it there before joining the hub's.
		await phone.disableWifi();

		await joinWifi(hostDevice, network.ssid, network.passphrase);
		hub = await spawnLocalHub('a');
		await chip().waitForStatus(
			'disconnected',
			UI_TIMEOUT,
			"the chip showed a hub before the phone joined the hub's LAN, so the " +
				'join would prove nothing',
		);
	});

	after(async () => {
		if (phone !== undefined) {
			try {
				await phone.enableWifi();
				await phone.forgetWifi(network.ssid);
			} catch {
				/* the phone is back on its usual network either way */
			}
		}
		if (hub !== undefined) await stopLocalHub(hub);
		if (network !== undefined) leaveWifi(network.ssid);
		if (mailboxKilled) await restartMailbox();
	});

	it('shows a hub already on the LAN within 2 seconds of the phone joining it', async () => {
		await phone.connectWifi(network.ssid, network.passphrase);
		await expectLocal("joining the hub's LAN");
	});

	it('drops the hub when it stops and shows it again when it starts under the foregrounded app', async () => {
		await stopLocalHub(hub);
		await expectNoHub('the hub stopped');
		hub = await restartLocalHub(hub);
		await expectLocal('the hub started');
	});

	it('shows a hub that started while the app was in the background', async () => {
		await stopLocalHub(hub);
		await expectNoHub('the hub stopped');
		await phone.backgroundApp();
		await phone.pause(BACKGROUNDED_MS);
		hub = await restartLocalHub(hub);
		await phone.startApp();
		await phone.groupChatPage.ready();
		await expectLocal('coming back to the foreground');
	});

	it('shows the hub again after each of several restarts', async () => {
		for (let restart = 1; restart <= RESTARTS; restart++) {
			await stopLocalHub(hub);
			await expectNoHub(`restart ${restart}: the hub stopped`);
			hub = await restartLocalHub(hub);
			await expectLocal(`restart ${restart}: the hub started`);
		}
	});

	it('shows the hub again within 2 seconds of the phone rejoining the LAN', async () => {
		await phone.disableWifi();
		await expectNoHub('the phone left the LAN');
		await phone.connectWifi(network.ssid, network.passphrase);
		await expectLocal('rejoining the LAN');
	});

	it('drops the hub when its host leaves the LAN and shows it again when the host rejoins', async () => {
		leaveWifi(network.ssid);
		await expectNoHub("the hub's host left the LAN");
		await joinWifi(hostDevice, network.ssid, network.passphrase);
		await expectLocal("the hub's host rejoined the LAN");
	});

	it('keeps showing the hub past the mDNS record TTL', async () => {
		const until = Date.now() + (MDNS_RECORD_TTL_S + 10) * 1_000;
		while (Date.now() < until) {
			await phone.pause(Math.min(10_000, until - Date.now()));
			expect(await chip().status()).toBe('local');
		}
	});

	it('shows the hub again after a Wi-Fi bounce', async () => {
		await phone.cycleWifi(WIFI_DOWN_MS);
		// The supplicant picks whatever saved network scores best; elsewhere the
		// hub is reached over another LAN and the bounce proves nothing.
		const { ssid } = await phone.wifiInfo();
		if (ssid !== network.ssid) {
			throw new Error(
				`the phone came back on "${ssid}", not "${network.ssid}"; re-run with the hub's network scoring best`,
			);
		}
		await expectLocal('Wi-Fi came back');
	});
});
