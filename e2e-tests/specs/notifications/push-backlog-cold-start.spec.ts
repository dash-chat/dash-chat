import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

// More than the 16-slot buffer between a p2panda topic stream and the node
// actor. The push extension stores these operations under its own cursor, so
// the app replays all of them when it next opens the chat's topic.
const BACKLOG = 200;

describe('Cold start after a push backlog', () => {
	let rex: Agent;
	let sam: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[rex, sam] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'any' },
		]);
		notifications = notificationHelperFor(rex);
		await createProfiles({ Rex: rex, Sam: sam });
		await exchangeContacts([rex, sam]);
	});

	it('opens and stays usable after many messages arrived while it was quit', async function () {
		this.timeout(20 * 60_000);
		await rex.directChatPage.back.click();
		await rex.homePage.ready();
		await rex.pause(5_000);
		await rex.stopApp();

		// Sent with Notification Center open: the pushes then land in its list
		// instead of as banners, which a backlog this long keeps on screen long
		// enough to block pulling Notification Center down afterwards.
		const marker = `BACKLOG_${Date.now()}`;
		await notifications.readingDelivered(async read => {
			for (let i = 1; i < BACKLOG; i++) {
				await sam.directChatPage.composer.sendMessage(`backlog ${i}`);
			}
			await sam.directChatPage.composer.sendMessage(`last ${marker}`);
			await rex.waitUntil(
				async () =>
					(await read()).some(n => n.texts.some(t => t.includes(marker))),
				{
					timeout: 60_000,
					timeoutMsg: `No notification containing "${marker}" arrived`,
				},
			);
		});

		await rex.startApp();
		await rex.homePage.ready();
		await rex.homePage.chatListItem('Sam').click();
		await rex.directChatPage.ready();
		await rex.directChatPage.messages.waitForMessage(`last ${marker}`);

		await rex.directChatPage.composer.sendMessage('caught up');
		await sam.directChatPage.messages.waitForMessage('caught up');
	});
});
