import { APP_BUNDLE_ID, APP_STATE_FOREGROUND } from '../setup/platforms/ios';
import {
	forgetIosWifi,
	iosAddedSsids,
	iosWifiInfo,
} from '../setup/platforms/ios-wifi';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { wifiNetworks } from '../setup/test-env';
import { type WifiInfo, waitForWifi } from '../setup/wifi';

const ROUNDS = 3;

/** Start an `add` and leave it running: iOS puts a "join?" alert over the
 *  webview for it, which the session's alert monitor answers meanwhile. */
function startAdd(
	agent: Agent,
	ssid: string,
	passphrase: string,
): Promise<void> {
	return agent.execute(
		(ssid: string, passphrase: string) => {
			void window.__test.wifi.add(ssid, passphrase);
		},
		ssid,
		passphrase,
	);
}

/** The interface once `done` holds of it. A read is refused while iOS has
 *  a "join?" alert over the webview; that passes. */
async function settle(
	agent: Agent,
	what: string,
	done: (info: WifiInfo) => boolean,
): Promise<WifiInfo> {
	const started = Date.now();
	let last: WifiInfo = { ssid: '', address: '', network: '' };
	await waitForWifi(
		async () => {
			last = await iosWifiInfo(agent).catch(() => last);
			return done(last) ? 'settled' : '';
		},
		`${what}: never settled; last ${JSON.stringify(last)}`,
	);
	console.log(
		`[wifi] ${what} in ${Date.now() - started}ms: ${JSON.stringify(last)}`,
	);
	return last;
}

async function inForeground(agent: Agent): Promise<boolean> {
	return (
		Number(
			await agent.execute('mobile: queryAppState', { bundleId: APP_BUNDLE_ID }),
		) === APP_STATE_FOREGROUND
	);
}

describe('Wi-Fi control plugin', () => {
	let agent: Agent;
	let lab: { ssid: string; passphrase: string };
	let home: WifiInfo;

	before(async function () {
		this.timeout(20 * 60_000);
		const networks = wifiNetworks();
		if (networks.length === 0) this.skip();
		lab = networks[0];
		[agent] = await setupAgents(this, [{ platform: 'ios' }]);
		home = await agent.wifiInfo();
	});

	it('starts with an address on a network it did not configure', () => {
		expect(home.address).not.toBe('');
		expect(home.network).not.toBe('');
		expect(home.ssid).toBe('');
	});

	it('adds, joins and forgets the lab network without leaving the foreground', async function () {
		this.timeout(20 * 60_000);
		for (let round = 1; round <= ROUNDS; round++) {
			// `add` only registers the network and lets the OS join in its own
			// time; `joinWifi` (the driver over `requestJoin`) returns once the
			// phone is on it, so its settle is a single read.
			const op = round === 1 ? 'add' : 'join';
			if (op === 'add') await startAdd(agent, lab.ssid, lab.passphrase);
			else await agent.joinWifi(lab.ssid, lab.passphrase);
			const onLab = await settle(
				agent,
				`${op} ${lab.ssid} #${round}`,
				info => info.ssid === lab.ssid && info.address !== '',
			);
			expect(onLab.network).not.toBe(home.network);
			expect(await iosAddedSsids(agent)).toEqual([lab.ssid]);
			expect(await inForeground(agent)).toBe(true);

			const started = Date.now();
			const back = await agent.leaveWifi();
			console.log(
				`[wifi] leave ${lab.ssid} #${round} in ${Date.now() - started}ms: ${back.address}`,
			);
			expect(back.network).toBe(home.network);
			expect(await iosAddedSsids(agent)).toEqual([]);
			expect(await inForeground(agent)).toBe(true);
		}
	});

	it('joins from off the air, and leaves from off the air back to its own network', async function () {
		this.timeout(20 * 60_000);
		await agent.disableWifi();
		expect((await agent.wifiInfo()).address).toBe('');
		const onLab = await agent.joinWifi(lab.ssid, lab.passphrase);
		expect(onLab).not.toBe('');
		expect((await agent.wifiInfo()).ssid).toBe(lab.ssid);

		await forgetIosWifi(agent);
		await agent.disableWifi();
		expect(await iosAddedSsids(agent)).toEqual([]);
		const back = await agent.leaveWifi();
		expect(back.network).toBe(home.network);
	});
});
