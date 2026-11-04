/**
 * Background mode's promise: a backgrounded Android app still notifies about
 * messages while offline. Both phones sit on a Wi-Fi network with no internet
 * and the mailbox is dead, so no push can be sent and the message can only cross
 * over a direct connection to the node background mode keeps alive.
 *
 * Needs two physical phones without mobile data, and a network in
 * E2E_WIFI_NETWORKS other than the one the phones are on; skips otherwise.
 */
import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { BACKGROUND_NETWORK_CUTOFF_MS } from '../../helpers/timeouts';
import { isRemoteMailbox, killMailbox } from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';
import { wifiNetworks } from '../../setup/test-env';

describe('Offline notifications on Android (background mode)', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		const [network] = wifiNetworks();
		if (network === undefined || isRemoteMailbox()) this.skip();
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'phone' },
		]);
		// The check below only throws on a `true`, so a probe that can never say
		// yes would let this spec pass on a network with full internet.
		if (!(await receiver.hasInternet())) {
			throw new Error('the internet probe says the home network is offline');
		}
		await killMailbox();
		notifications = notificationHelperFor(receiver);
		await notifications.clear();
		await createProfiles({ Rex: receiver, Sam: sender });
		await exchangeContacts([receiver, sender]);
		for (const agent of [sender, receiver]) {
			await agent.joinWifi(network.ssid, network.passphrase);
			if (await agent.hasInternet()) {
				throw new Error(
					`a phone on "${network.ssid}" reaches the internet; this spec needs a network with no upstream and phones without mobile data`,
				);
			}
		}
	});

	it('notifies about a message sent over the offline network while backgrounded', async () => {
		// With the app on screen first, so a failure below is about the background
		// and not about these two phones never reaching each other on this network.
		await sender.directChatPage.composer.sendMessage('still on screen');
		await receiver.directChatPage.messages.waitForMessage('still on screen');

		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.homePage.settingsLink.click();
		await receiver.settingsPage.ready();
		await receiver.settingsPage.offlineLink.click();
		await receiver.offlinePage.ready();
		await receiver.offlinePage.setBackgroundModeEnabled(true);

		await receiver.backgroundApp();
		await receiver.pause(BACKGROUND_NETWORK_CUTOFF_MS);

		const message = 'sent while you were in the background';
		await sender.directChatPage.composer.sendMessage(message);
		await expect(notifications).toHaveDelivered({ body: message });

		await notifications.tapNotification(message);
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await receiver.directChatPage.messages.waitForMessage(message);
	});
});
