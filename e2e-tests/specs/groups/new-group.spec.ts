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

	// it('creates a new group with no members except the creator', async () => {
	// 	await agent
	// 		.onHomePage()
	// 		.ready()
	// 		.then(p => p.clickNewMessage());
	// 	await agent
	// 		.onNewMessagePage()
	// 		.ready()
	// 		.then(p => p.clickNewGroup());
	// 	await agent
	// 		.onNewGroupPage()
	// 		.onAddMembersStep()
	// 		.ready()
	// 		.then(p => p.clickNext());
	// 	await agent
	// 		.onNewGroupPage()
	// 		.onGroupInfoStep()
	// 		.ready()
	// 		.then(p => p.clickCreate());

	// 	await agent
	// 		.onHomePage()
	// 		.ready()
	// 		.then(p => p.expectChatListToHaveGroupChatWithName('mygroup'));
	// });

	it('creates a new group with another member', async () => {
		// Create another agent and exchange contact info
		const agent2 = makeAgent(browser.getInstance('agent2'));
		await waitForTestUtils(agent2);
		await agent2.createProfile('Bob', 'Test');
		await agent.navigateToAddContact();
		await agent2.navigateToAddContact();
		const code1 = await agent.getContactCode();
		const code2 = await agent2.getContactCode();
		if (!code1 || !code2) throw new Error('contact code missing');
		await agent.addContact(code2);
		await agent2.addContact(code1);

		// Create group with both members
		await agent.onHomePage().ready();
		await agent.onHomePage().clickNewMessage();

		await agent.onNewMessagePage().ready();
		await agent.onNewMessagePage().clickNewGroup();

		const addMembersStep = await agent
			.onNewGroupPage()
			.onAddMembersStep()
			.ready();
		await addMembersStep.addContactByName('Bob');
		await addMembersStep.clickNext();

		await agent.onNewGroupPage().onGroupInfoStep().ready();
		await agent.onNewGroupPage().onGroupInfoStep().clickCreate();

		await agent.onHomePage().ready();
		await agent.onHomePage().expectChatListToHaveGroupChatWithName('mygroup');
	});
});
