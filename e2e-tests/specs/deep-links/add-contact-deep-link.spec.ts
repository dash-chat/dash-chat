import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Deep links', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await Promise.all([
			agent1.createProfilePage.createProfile('Alice', 'Test'),
			agent2.createProfilePage.createProfile('Bob', 'Test'),
		]);
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
			// The dash-chat:// scheme is only registered with the OS on desktop,
			// so mobile has no native delivery path for it — inject straight into
			// the app's routing to cover the scheme parsing on every platform.
			await agent1.injectDeepLink('dash-chat://add-contact/invalidcode');
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
				async () => agent2.homePage.hasChatListItem('Alice Test'),
				{ timeout: 15_000 },
			);
			await agent2.homePage.openChat('Alice Test');
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
	});
});
