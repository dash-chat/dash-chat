/**
 * Local hub discovery on the paths it is known to break on, walked in order:
 * a hub starting under the open app, stopping with and without its mDNS
 * goodbye, restarting several times in a row, starting while the app is
 * closed or backgrounded, the phone leaving and rejoining its LAN, a sit-still
 * past the mDNS record TTL, a Wi-Fi bounce, and the hub's host joining the
 * LAN the phone is already on. The hub is the standalone
 * `mailbox-local-server` on the host, heard on whatever LAN the host is on —
 * a phone only has to be on the host's Wi-Fi; the cloud mailbox is killed so
 * the chip is on screen and reads the hub. The cases that need a phone's
 * Wi-Fi or background skip themselves on desktop, and the host-joins case
 * also needs a network from E2E_WIFI_NETWORKS the host is not on yet.
 * local-hub-discovery-stress walks these and the network moves at random on
 * real access points.
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

	let agent: Agent;
	let hub: LocalHub;
	let mailboxKilled = false;

	const chip = () => agent.groupChatPage.connectionStatusIndicator;

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

	/** The supplicant picks whatever saved network scores best; elsewhere the
	 *  hub is reached over another LAN and the move proves nothing. */
	async function expectBackOn(ssid: string): Promise<void> {
		const { ssid: now } = await agent.wifiInfo();
		if (now !== ssid) {
			throw new Error(
				`the phone came back on "${now}", not "${ssid}"; re-run with the host's network scoring best`,
			);
		}
	}

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		// An emulator is NAT'd off the host's LAN, so no hub can reach it.
		if (agent.platform === 'android-emulator') this.skip();
		await killMailbox();
		mailboxKilled = true;
		await agent.createProfilePage.createProfile('Alice', 'Hub');
		await createGroup(agent, 'Solo Group', []);
		await chip().waitForStatus(
			'disconnected',
			UI_TIMEOUT,
			'the chip showed a hub before the spec started one, so the LAN is ' +
				'not clean and the run would prove nothing',
		);
	});

	after(async () => {
		if (hub !== undefined) await stopLocalHub(hub);
		if (mailboxKilled) await restartMailbox();
	});

	it('shows a hub within 2 seconds of it starting', async () => {
		hub = await spawnLocalHub('a');
		await expectLocal('the hub started');
	});

	it('drops the hub when it stops and shows it again when it starts', async () => {
		await stopLocalHub(hub);
		await expectNoHub('the hub stopped');
		hub = await restartLocalHub(hub);
		await expectLocal('the hub started');
	});

	it('drops a hub that was killed without its goodbye', async () => {
		await stopLocalHub(hub, 'SIGKILL');
		await expectNoHub('the hub was killed');
		hub = await restartLocalHub(hub);
		await expectLocal('the hub started');
	});

	it('shows the hub again after each of several restarts', async () => {
		for (let restart = 1; restart <= RESTARTS; restart++) {
			await stopLocalHub(hub);
			await expectNoHub(`restart ${restart}: the hub stopped`);
			hub = await restartLocalHub(hub);
			await expectLocal(`restart ${restart}: the hub started`);
		}
	});

	it('shows a hub that started while the app was closed', async () => {
		await stopLocalHub(hub);
		await expectNoHub('the hub stopped');
		await agent.stopApp();
		hub = await restartLocalHub(hub);
		await agent.startApp();
		await agent.homePage.ready();
		await agent.homePage.chatListItem('Solo Group').click();
		await agent.groupChatPage.ready();
		await expectLocal('the app came back');
	});

	it('shows a hub that started while the app was in the background', async function () {
		if (!agent.isMobile) this.skip();
		await stopLocalHub(hub);
		await expectNoHub('the hub stopped');
		await agent.backgroundApp();
		await agent.pause(BACKGROUNDED_MS);
		hub = await restartLocalHub(hub);
		await agent.startApp();
		await agent.groupChatPage.ready();
		await expectLocal('coming back to the foreground');
	});

	it('shows the hub again within 2 seconds of the phone rejoining the LAN', async function () {
		if (!agent.isMobile) this.skip();
		const { ssid } = await agent.wifiInfo();
		await agent.disableWifi();
		await expectNoHub('the phone left the LAN');
		await agent.enableWifi();
		await expectBackOn(ssid);
		await expectLocal('rejoining the LAN');
	});

	it('keeps showing the hub past the mDNS record TTL', async () => {
		const until = Date.now() + (MDNS_RECORD_TTL_S + 10) * 1_000;
		while (Date.now() < until) {
			await agent.pause(Math.min(10_000, until - Date.now()));
			expect(await chip().status()).toBe('local');
		}
	});

	it('shows the hub again after a Wi-Fi bounce', async function () {
		if (!agent.isMobile) this.skip();
		const { ssid } = await agent.wifiInfo();
		await agent.cycleWifi(WIFI_DOWN_MS);
		await expectBackOn(ssid);
		await expectLocal('Wi-Fi came back');
	});

	it('shows the hub when its host joins the LAN the phone is already on', async function () {
		const [network] = wifiNetworks();
		if (!agent.isMobile || network === undefined) this.skip();
		const device = wifiDevice();
		if (device === null) throw new Error('the host has no Wi-Fi card');
		try {
			await agent.connectWifi(network.ssid, network.passphrase);
			await expectNoHub('the phone moved to a LAN the hub is not on');
			await joinWifi(device, network.ssid, network.passphrase);
			await expectLocal("the hub's host joined the phone's LAN");
		} finally {
			leaveWifi(network.ssid);
			await agent.forgetWifi(network.ssid);
		}
	});
});
