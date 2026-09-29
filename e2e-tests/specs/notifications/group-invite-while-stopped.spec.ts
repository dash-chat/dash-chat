import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Being added to a group has to be announced on a device whose app is not
 * running.
 *
 * The invitation reaches the invitee as a JoinGroup in the pair's direct chat,
 * the only topic of the two the invitee is subscribed to. The group's own
 * topic, which carries the operation the announcement is built from, is one
 * the invitee's device only learns of from that invitation.
 */
describe('Group invite while the app is stopped', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'any' },
		]);
		notifications = notificationHelperFor(receiver);
		await createProfiles({ Rex: receiver, Sam: sender });
		await exchangeContacts([receiver, sender]);
		await backToHome([receiver, sender]);
		await notifications.clear();
	});

	afterEach(async function () {
		if (this.currentTest?.state === 'failed') await notifications.recover();
	});

	it('announces the group the receiver was added to', async () => {
		const group = 'Invited group';
		const body = await sender.tr('someoneAddedYouToTheGroup', { name: 'Sam' });
		const titles = [group, await sender.tr('newGroup')];

		await receiver.stopApp();
		await createGroup(sender, group, ['Rex']);

		const shown = await notifications.readingDelivered(read =>
			receiver
				.waitUntil(async () => {
					const all = await read();
					return all.some(n => n.body === body) ? all : false;
				})
				.catch(() => read()),
		);
		expect(shown.map(n => n.body)).toContain(body);
		expect(titles).toContain(shown.find(n => n.body === body)?.title);
	});

	it('announces a message posted in that group while the app is still stopped', async () => {
		const text = `after the invite ${Date.now()}`;
		await sender.groupChatPage.composer.sendMessage(text);

		const shown = await notifications.readingDelivered(read =>
			receiver
				.waitUntil(async () => {
					const all = await read();
					return all.some(n => n.body === text) ? all : false;
				})
				.catch(() => read()),
		);
		expect(shown.map(n => n.body)).toContain(text);
		expect(shown.find(n => n.body === text)?.title).toBe('Sam');
	});

	it('announces being added back after being removed while the app is still stopped', async () => {
		const body = await sender.tr('someoneAddedYouToTheGroup', { name: 'Sam' });

		await sender.groupChatPage.infoLink.click();
		await sender.groupInfoPage.ready();
		await sender.groupInfoPage.memberItem('Rex').click();
		await sender.groupInfoPage.removeMemberButton.click();
		await sender.groupInfoPage.removeMemberConfirmButton.click();
		await sender.groupInfoPage
			.memberItem('Rex')
			.waitForExist({ reverse: true });
		await sender.groupInfoPage.addMembersLink.click();
		await sender.addMembersPage.ready();
		await sender.addMembersPage.addContactByName('Rex');
		await sender.addMembersPage.addButton.click();

		// The first add's announcement is still in the shade.
		const invitesShown = (all: { body: string }[]) =>
			all.filter(n => n.body === body).length;
		const shown = await notifications.readingDelivered(read =>
			receiver
				.waitUntil(async () => {
					const all = await read();
					return invitesShown(all) === 2 ? all : false;
				})
				.catch(() => read()),
		);
		expect(invitesShown(shown)).toBe(2);
	});
});
