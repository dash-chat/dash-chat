import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Deep links', () => {
	let agent: Agent;

	before(async () => {
		agent = await setupAgent('agent1');
		await agent.createProfilePage.createProfile('Alice', 'Test');
	});

	describe('add-contact deep link', () => {
		it('shows an error toast for a totally invalid contact code', async () => {
			await agent.handleDeepLink('dash-chat://add-contact/invalidcode');
			await agent.addContactPage.ready();
			await agent.toast.expectMessage(
				await agent.tr('errorAddContactInvalidCode'),
			);
			await agent.addContactPage.back.click();
			await agent.homePage.ready();
		});

		it('shows an error toast for an https deep link with an invalid contact code', async () => {
			await agent.handleDeepLink(
				'https://dashchat.org/add-contact/invalidcode',
			);
			await agent.addContactPage.ready();
			await agent.toast.expectMessage(
				await agent.tr('errorAddContactInvalidCode'),
			);
			await agent.addContactPage.back.click();
			await agent.homePage.ready();
		});
	});
});
