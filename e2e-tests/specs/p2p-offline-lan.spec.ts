/**
 * Peer-to-peer sync on a LAN with no internet from the first launch: both
 * phones join an E2E_WIFI_NETWORKS access point with no upstream before their
 * apps start, and the cloud mailbox is down, so neither the relay nor a
 * mailbox can carry anything. Contact exchange, text messages and media can
 * only cross over a direct connection to a peer discovered over mDNS.
 *
 * Needs two physical phones without mobile data, and a network in
 * E2E_WIFI_NETWORKS other than the one the phones are on; skips otherwise:
 *   PLATFORMS=android,android just e2e run p2p-offline-lan
 *   PLATFORMS=ios,ios just e2e run p2p-offline-lan
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { type WifiNetwork, wifiNetworks } from '../setup/test-env';

/** Close the app, move the phone onto `network`, and relaunch it there wiped
 *  back to first launch, so its node knows nothing from a network with
 *  internet. */
async function relaunchOn(agent: Agent, network: WifiNetwork): Promise<void> {
	await agent.stopApp();
	await agent.connectWifi(network.ssid, network.passphrase);
	// On iOS this brings the app up on the old account to ask from its
	// webview, so the wipe below is what actually leaves it at first launch.
	if (await agent.hasInternet()) {
		throw new Error(
			`"${network.ssid}" reaches the internet; this spec needs a network with no upstream`,
		);
	}
	await agent.clearAppData();
	await agent.startApp();
}

describe('P2P sync on a LAN with no internet', () => {
	let alice: Agent;
	let bob: Agent;
	let offline: WifiNetwork | undefined;
	let mailboxKilled = false;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'phone' },
			{ platform: 'phone' },
		]);
		const home = (await alice.wifiInfo()).ssid;
		// A positive control for the probe `relaunchOn` leans on. It only throws
		// on a `true`, so a probe that can never say yes — a retired endpoint, a
		// CORS policy change, a webview that runs the script but cannot fetch —
		// would let this whole spec pass on a network with full internet,
		// proving nothing about mDNS. The phones start on the lab's network,
		// which has upstream, so the probe has to say so here.
		if (!(await alice.hasInternet())) {
			throw new Error(
				`the internet probe says "${home}" has no upstream, so it cannot ` +
					'tell an offline network from a broken probe. This spec needs to ' +
					'start on a network with internet.',
			);
		}
		const network = wifiNetworks().find(n => n.ssid !== home);
		if (network === undefined) this.skip();
		offline = network;
		await killMailbox();
		mailboxKilled = true;
		await Promise.all([alice, bob].map(agent => relaunchOn(agent, network)));
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
	});

	after(async () => {
		if (offline !== undefined) {
			await alice.forgetWifi(offline.ssid);
			await bob.forgetWifi(offline.ssid);
		}
		if (mailboxKilled) await restartMailbox();
	});

	it('syncs a text message Alice → Bob', async () => {
		await alice.directChatPage.composer.sendMessage('hello with no internet');
		await bob.directChatPage.messages.waitForMessage('hello with no internet');
	});

	it('syncs a reply Bob → Alice', async () => {
		await bob.directChatPage.composer.sendMessage('reply with no internet');
		await alice.directChatPage.messages.waitForMessage(
			'reply with no internet',
		);
	});

	it('syncs a photo message', async () => {
		const { composer } = alice.directChatPage;
		await composer.attachPhotos('lanphoto');
		await composer.type('photo with no internet');
		await composer.send();
		await alice.directChatPage.messages.waitForPhotoMessage('lanphoto');
		await bob.directChatPage.messages.waitForMessage('photo with no internet');
		await bob.directChatPage.messages.waitForPhotoMessage('lanphoto');
	});
});
