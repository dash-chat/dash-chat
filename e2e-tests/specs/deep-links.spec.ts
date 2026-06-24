import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Deep links', () => {
	let agent: Agent;

	before(async () => {
		agent = await setupAgent('agent1');
		await agent.createProfilePage.createProfile('Alice', 'Test');
	});

	describe('add-contact deep link', () => {
		it('shows an error toast for a totally invalid contact code', async () => {
			// Simulate the deep link handler navigating to this URL with an invalid code.
			await agent.goto('/new-message/add-contact?code=invalidcode');
			await agent.addContactPage.ready();
			await agent.toast.expectMessage(
				await agent.tr('errorAddContactInvalidCode'),
			);
		});
	});
});
