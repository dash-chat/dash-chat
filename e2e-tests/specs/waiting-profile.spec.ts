import { S } from '../../ui/tests/selectors';
import { type Agent, setupAgent } from '../helpers/setup-agents';

describe('Waiting-for-profile placeholder', () => {
	let agent1: Agent;
	let agent2: Agent;
	let waitingText: string;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfile('Alice', 'Test');
		await agent2.createProfile('Bob', 'Test');
		waitingText = await agent2.tr('waitingForProfile');
	});

	it('shows the placeholder on direct-chat after one-sided contact addition', async () => {
		await agent1.navigateToAddContact();
		const code1 = await agent1.getContactCode();
		if (!code1) throw new Error('agent1 contact code missing');

		await agent2.navigateToAddContact();
		await agent2.addContact(code1);

		await agent2.waitForText(S.directChat.peerHeader, waitingText);
		await agent2.waitForText(S.directChat.settingsLink, waitingText);
	});

	it('shows the placeholder on chat-settings', async () => {
		await agent2.click(S.directChat.settingsLink);
		await agent2.waitForText(S.chatSettings.peerHeader, waitingText);
	});

	it('shows the placeholder on the home chat-list row', async () => {
		await agent2.goto('/');
		await agent2.waitForText(S.home.chatRow, waitingText);
	});
});
