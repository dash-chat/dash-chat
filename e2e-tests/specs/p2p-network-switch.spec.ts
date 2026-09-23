/**
 * The smallest on-device reproduction of p2p sync not resuming after a
 * network change: two phones on the same LAN, no cloud mailbox and no hub, so
 * a message can only cross over a direct iroh connection. One phone drops off
 * Wi-Fi for a while — on screen, in the background or killed — while either
 * side composes a message; once it holds its address again the message has to
 * cross within seconds, and so has one sent once it is back. The last case
 * walks both phones onto another Wi-Fi network with the app on screen, so it
 * needs a second network in E2E_WIFI_NETWORKS (see e2e-tests/.env.example)
 * and skips without one.
 *
 * Skips itself unless E2E_STRESS=1. Either phone platform will do — both
 * drive their Wi-Fi through the harness:
 *   PLATFORMS=android,android E2E_STRESS=1 just e2e run p2p-network-switch
 *   PLATFORMS=ios,ios E2E_STRESS=1 just e2e run p2p-network-switch
 */
import { createProfilesAndExchangeContacts } from '../helpers/flows/exchange-contacts';
import { stampedLog } from '../helpers/utils';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { type WifiNetwork, wifiNetworks } from '../setup/test-env';

const AWAY_MS = 60_000;
/** Past the peer's QUIC idle timeout on every old session, so the return
 *  also has to get through its stale-session cleanup. */
const LONG_AWAY_MS = 300_000;
/** Peer discovery and sync, once the phone holds its address again. */
const DISCOVERY_AND_SYNC_MS = 4_000;

/** `text` must reach `receiver` within [`DISCOVERY_AND_SYNC_MS`] of `since`. */
async function expectArrival(
	receiver: Agent,
	text: string,
	since: number,
): Promise<void> {
	await receiver.waitUntil(
		async () =>
			(await receiver.directChatPage.messages.messageWithText(text)) !== null,
		{
			interval: 50,
			timeout: DISCOVERY_AND_SYNC_MS,
			timeoutMsg: `"${text}" did not arrive within ${DISCOVERY_AND_SYNC_MS / 1_000}s`,
		},
	);
	stampedLog(`"${text}" arrived ${Date.now() - since}ms into the budget`);
}

describe('Pure p2p sync across a network switch', function () {
	this.timeout(LONG_AWAY_MS + 120_000);

	let alice: Agent;
	let bob: Agent;
	let mailboxKilled = false;
	let otherNetwork: WifiNetwork | undefined;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (isRemoteMailbox()) this.skip();
		await killMailbox();
		mailboxKilled = true;
		[alice, bob] = await setupAgents(this, [
			{ platform: 'phone' },
			{ platform: 'phone' },
		]);
		// An earlier run that died mid-case leaves a phone off the air.
		await alice.enableWifi();
		await bob.enableWifi();
		await createProfilesAndExchangeContacts({ Alice: alice, Bob: bob });
		// A round trip on the LAN before the move, so a later failure is about
		// the return and not about p2p never having worked between these two.
		await bob.directChatPage.composer.sendMessage('hello before leaving');
		await expectArrival(alice, 'hello before leaving', Date.now());
	});

	after(async () => {
		if (otherNetwork !== undefined) {
			await alice.forgetWifi(otherNetwork.ssid);
			await bob.forgetWifi(otherNetwork.ssid);
		}
		if (mailboxKilled) await restartMailbox();
	});

	it('receives a message the peer sent while it was off Wi-Fi, once back on the LAN', async () => {
		const text = 'sent while you were away';
		await alice.disableWifi();
		stampedLog('Alice off Wi-Fi');
		await bob.directChatPage.composer.sendMessage(text);
		await alice.pause(AWAY_MS);
		expect(await bob.directChatPage.messageStatusFor(text)).not.toBe(
			'delivered',
		);
		await alice.enableWifi();
		stampedLog('Alice holds her address again');
		await expectArrival(alice, text, Date.now());
	});

	it('receives a message the peer sent during a long absence, once back on the LAN', async () => {
		const text = 'sent during your long absence';
		await alice.disableWifi();
		stampedLog('Alice off Wi-Fi');
		await bob.directChatPage.composer.sendMessage(text);
		await alice.pause(LONG_AWAY_MS);
		await alice.enableWifi();
		stampedLog('Alice holds her address again');
		await expectArrival(alice, text, Date.now());
	});

	it('receives a message composed while both phones were off Wi-Fi, once both are back', async () => {
		const text = 'composed while we were both away';
		await alice.disableWifi();
		await bob.disableWifi();
		stampedLog('both phones off Wi-Fi');
		await bob.directChatPage.composer.sendMessage(text);
		await alice.pause(AWAY_MS);
		await alice.enableWifi();
		await bob.enableWifi();
		stampedLog('both phones hold their addresses again');
		await expectArrival(alice, text, Date.now());
	});

	it('delivers a message it composed while off Wi-Fi, once back on the LAN', async () => {
		const text = 'composed while I was away';
		await alice.disableWifi();
		stampedLog('Alice off Wi-Fi');
		await alice.directChatPage.composer.sendMessage(text);
		await alice.pause(AWAY_MS);
		await alice.enableWifi();
		stampedLog('Alice holds her address again');
		await expectArrival(bob, text, Date.now());
	});

	it('receives a message the peer sent shortly after it was back on the LAN', async () => {
		const text = 'sent once you were back';
		await alice.disableWifi();
		stampedLog('Alice off Wi-Fi');
		await alice.pause(AWAY_MS);
		await alice.enableWifi();
		stampedLog('Alice holds her address again');
		await alice.pause(DISCOVERY_AND_SYNC_MS);
		await bob.directChatPage.composer.sendMessage(text);
		await expectArrival(alice, text, Date.now());
	});

	it('receives a message the peer sent while it was backgrounded off Wi-Fi, once back on the LAN and on screen', async () => {
		const text = 'sent while you were in the background';
		await alice.backgroundApp();
		await alice.disableWifi();
		stampedLog('Alice in the background and off Wi-Fi');
		await bob.directChatPage.composer.sendMessage(text);
		await alice.pause(AWAY_MS);
		await alice.enableWifi();
		await alice.startApp();
		stampedLog('Alice holds her address again and is on screen');
		const since = Date.now();
		await alice.directChatPage.ready();
		await expectArrival(alice, text, since);
	});

	it('receives a message the peer sent while it was killed off Wi-Fi, once relaunched on the LAN', async () => {
		const text = 'sent while your app was dead';
		await alice.stopApp();
		await alice.disableWifi();
		stampedLog('Alice killed and off Wi-Fi');
		await bob.directChatPage.composer.sendMessage(text);
		await alice.pause(AWAY_MS);
		await alice.enableWifi();
		await alice.startApp();
		stampedLog('Alice holds her address again and is relaunched');
		const since = Date.now();
		await alice.homePage.ready();
		await alice.homePage.chatListItem('Bob').click();
		await alice.directChatPage.ready();
		await expectArrival(alice, text, since);
	});

	it('receives a message the peer sent once both phones had moved to another Wi-Fi network', async function () {
		const home = (await alice.wifiInfo()).ssid;
		otherNetwork = wifiNetworks().find(n => n.ssid !== home);
		if (otherNetwork === undefined) this.skip();
		const text = 'sent once we had both moved network';
		await alice.connectWifi(otherNetwork.ssid, otherNetwork.passphrase);
		await bob.connectWifi(otherNetwork.ssid, otherNetwork.passphrase);
		stampedLog(`both phones on ${otherNetwork.ssid}`);
		await bob.directChatPage.composer.sendMessage(text);
		await expectArrival(alice, text, Date.now());
	});
});
