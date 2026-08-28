import { deleteAccount } from '../helpers/flows/delete-account';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Delete account', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		await agent.createProfilePage.createProfile('Delete', 'Account');
	});

	it('wipes all data and restarts at first launch when confirmed', async () => {
		await deleteAccount(agent);
		await agent.createProfilePage.createProfile('Fresh', 'Start');
	});
});
