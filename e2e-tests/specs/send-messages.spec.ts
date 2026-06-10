import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Full messaging flow', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('creates profiles on both agents', async () => {
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
	});

	it('exchanges contact codes between agents', async () => {
		await exchangeContacts(agent1, agent2);
	});

	it('sends a message from Alice to Bob', async () => {
		await agent1.directChatPage.sendMessage('Hello from Alice!');
		await agent1.directChatPage.waitForMessage('Hello from Alice!');
		await agent2.directChatPage.waitForMessage('Hello from Alice!');
	});

	it('sends a reply from Bob to Alice', async () => {
		await agent2.directChatPage.sendMessage('Hello from Bob!');
		await agent2.directChatPage.waitForMessage('Hello from Bob!');
		await agent1.directChatPage.waitForMessage('Hello from Bob!');
	});
});
