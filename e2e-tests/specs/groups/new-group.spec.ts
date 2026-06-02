/**
 * New group creation E2E test.
 */
import {
	type Agent,
	makeAgent,
	waitForTestUtils,
} from '../../helpers/setup-agents';

describe('New group', () => {
	let agent: Agent;

	before(async () => {
		agent = makeAgent(browser.getInstance('agent1'));
		await waitForTestUtils(agent);
		await agent.enablePreviewFeatures();
		await agent.createProfile('Alice', 'Test');
	});

	it('creates a new group with no members except the creator', async () => {
		await agent
			.onHomePage()
			.ready()
			.then(p => p.clickNewMessage());
		await agent
			.onNewMessagePage()
			.ready()
			.then(p => p.clickNewGroup());
		await agent
			.onNewGroupPage()
			.onAddMembersStep()
			.ready()
			.then(p => p.clickNext());
		await agent
			.onNewGroupPage()
			.onGroupInfoStep()
			.ready()
			.then(p => p.clickCreate());

		await agent
			.onHomePage()
			.ready()
			.then(p => p.expectChatListToHaveGroupChatWithName('mygroup'));
	});

	it('creates a new group with another member', async () => {
		// Create another agent and exchange contact info
		const agent2 = makeAgent(browser.getInstance('agent2'));
		await waitForTestUtils(agent2);
		await agent2.createProfile('Bob', 'Test');
		await agent2.navigateToAddContact();
		const bob_code = await agent2.getContactCode();

		expect(bob_code).not.toBeNull();
		await agent2.navigateToAddContact();
		await agent.addContact(bob_code as string);

		// Create group with both members
		await agent
			.onHomePage()
			.ready()
			.then(p => p.clickNewMessage());
		await agent
			.onNewMessagePage()
			.ready()
			.then(p => p.clickNewGroup());
		await agent
			.onNewGroupPage()
			.onAddMembersStep()
			.ready()
			.then(p => p.addContactByName('Bob'));
		await agent
			.onNewGroupPage()
			.onGroupInfoStep()
			.ready()
			.then(p => p.clickCreate());

		await agent
			.onHomePage()
			.ready()
			.then(p => p.expectChatListToHaveGroupChatWithName('mygroup'));
	});
});
