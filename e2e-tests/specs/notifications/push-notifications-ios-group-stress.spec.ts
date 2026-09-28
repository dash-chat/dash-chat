import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/** Groups the Mac adds the iPhone to, one after another. Each invite races
 *  the push extension against the open app's own mailbox poll. */
const ROUNDS = 10;

/**
 * A group the iPhone is added to while its app is open must keep syncing in
 * that app. The invite reaches the iPhone twice: as a push, which wakes the
 * push extension, and through the app's own mailbox poll, and whichever
 * fetches it first joins the group. When the extension does, the group lands
 * in the shared database but the running app never subscribes to its topic,
 * so nothing posted in the group afterwards reaches the app until it is
 * relaunched.
 *
 * Skips itself unless E2E_STRESS=1, and unless push testing is available (a
 * Firebase service-account key plus a mobile agent). Run it with:
 *   PLATFORMS=ios,desktop just e2e run notifications/push-notifications-ios-group-stress
 */
// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so it must be set suite-wide: ROUNDS groups take well past the 300s
// default.
describe('Groups joined while the app is open', function () {
	this.timeout(1_800_000);

	let iphone: Agent;
	let mac: Agent;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (!pushTestingEnabled()) this.skip();
		[iphone, mac] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'desktop' },
		]);
		// Over a direct connection the Mac hands the invite to the app itself,
		// before any push can wake the extension.
		await iphone.disableP2p();
		await mac.disableP2p();
		await createProfiles({ Rex: iphone, Sam: mac });
		await exchangeContacts([iphone, mac]);
		await backToHome([iphone, mac]);
		// An extension process built before the contact existed does not fetch
		// the topics the invites arrive on.
		await iphone.killPushExtension();
	});

	it('keeps syncing every group the iPhone is added to', async () => {
		for (let round = 1; round <= ROUNDS; round++) {
			const name = `race ${String(round).padStart(String(ROUNDS).length, '0')}`;
			const message = `hello ${name}`;

			await createGroup(mac, name, ['Rex']);
			await iphone.homePage
				.chatListItem(name)
				.waitForExist({ timeout: SYNC_TIMEOUT });
			// iOS leaves the extension running about 5s after a push, and one that
			// joined polls the group meanwhile: the app would show what it fetches.
			await iphone.pause(10_000);
			await mac.groupChatPage.composer.sendMessage(message);

			await iphone.homePage.chatListItem(name).click();
			await iphone.groupChatPage.ready();
			await iphone.groupChatPage.messages.waitForMessage(message);

			await iphone.groupChatPage.back.click();
			await iphone.homePage.ready();
			await mac.groupChatPage.back.click();
			await mac.homePage.ready();
		}
	});
});
