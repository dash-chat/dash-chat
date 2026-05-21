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
		await agent.createProfile('Alice', 'Test');
	});

	it('navigates to the new-group page and types a group name', async () => {
		await agent
			.onHomePage()
			.ready()
			.then(p => p.clickNewMessage());
		await agent
			.onNewMessagePage()
			.ready()
			.then(p => p.clickNewGroup());
		await agent.onNewGroupPage().onAddMembersStep().ready();
	});
});
