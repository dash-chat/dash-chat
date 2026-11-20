import { createProfiles } from '../../helpers/flows/create-profiles';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Deep links', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: agent1, Bob: agent2 });
	});

	describe('add-contact deep link', () => {
		let link: string;

		it('shows an error toast for an invalid contact code without navigating', async () => {
			await agent1.handleDeepLink(
				'https://dashchat.org/add-contact/invalidcode',
			);
			await agent1.toast.expectMessage(
				await agent1.tr('errorAddContactInvalidLink'),
			);
			await agent1.homePage.ready();
		});

		it('shows an error toast for a scheme-based deep link with an invalid contact code', async () => {
			// Inject straight into the app's routing to cover the scheme parsing
			// the same way on every platform.
			await agent1.injectDeepLink('dash-chat://add-contact/invalidcode');
			await agent1.toast.expectMessage(
				await agent1.tr('errorAddContactInvalidLink'),
			);
			await agent1.homePage.ready();
		});

		it('delivers a scheme-based deep link through the OS on mobile', async function () {
			// The add-contact page on dashchat.org can only open the app through
			// this scheme: a tap on a same-domain https link stays in the browser.
			// Desktop e2e builds have no OS delivery path at all.
			if (agent1.platform === 'desktop') return this.skip();

			// The previous test's identical toast would otherwise pass this one,
			// even if the OS never delivered the link.
			await agent1.toast.root.waitForDisplayed({ reverse: true });

			await agent1.handleDeepLink('dash-chat://add-contact/invalidcode');
			await agent1.toast.expectMessage(
				await agent1.tr('errorAddContactInvalidLink'),
			);
			await agent1.homePage.ready();
		});

		it('opens a direct chat with the correct contact for a valid contact code', async () => {
			await agent2.homePage.newMessageButton.click();
			await agent2.newMessagePage.addContact.click();
			await agent2.addContactPage.ready();
			link = await agent2.addContactPage.getAddContactLink();

			await agent2.addContactPage.back.click();
			await agent2.newMessagePage.back.click();
			await agent2.homePage.ready();

			await agent1.handleDeepLink(link);
			await agent1.directChatPage.ready();

			await agent2.waitUntil(
				async () => agent2.homePage.hasChatListItem('Alice'),
				{ timeout: 15_000 },
			);
			await agent2.homePage.openChat('Alice');
			await agent2.waitUntil(() =>
				agent2.directChatPage.isContactRequestBannerVisible(),
			);
		});

		it('handles a deep link that launches the app from cold start', async function () {
			// Cold-start delivery (intent/universal link launching the app, read
			// back via getCurrent()) only exists on mobile; desktop e2e builds
			// have no OS delivery path at all.
			if (agent1.platform === 'desktop') return this.skip();

			await agent1.stopApp();
			await agent1.handleDeepLink(link);
			await agent1.startApp();
			await agent1.directChatPage.ready();
		});

		it('handles a scheme-based deep link that launches the app from cold start', async function () {
			// What someone who just installed the app hits when they tap "Add
			// contact" on the dashchat.org page.
			if (agent1.platform === 'desktop') return this.skip();

			await agent1.stopApp();
			await agent1.handleDeepLink(
				link.replace('https://dashchat.org/', 'dash-chat://'),
			);
			await agent1.startApp();
			await agent1.directChatPage.ready();
		});
	});
});
