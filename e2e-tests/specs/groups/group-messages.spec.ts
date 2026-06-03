import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Group messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.addContactByName('Bob');
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
	});

	it('renders messages from other group members with their avatar', async () => {
		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.sendMessage('Hello from Bob!');
		await agent2.groupChatPage.waitForMessage('Hello from Bob!');

		await agent1.groupChatPage.waitForMessage('Hello from Bob!');
		await agent1.waitUntil(
			async () =>
				(await agent1.groupChatPage.getAuthorInitials('Hello from Bob!')) ===
				'B',
			{ timeoutMsg: 'Avatar initials "B" did not appear on Bob\'s message' },
		);
	});

	it('shows the sender name only on the first message of a cluster, with a consistent color across clusters', async () => {
		await agent1.groupChatPage.sendMessage('Alice break A');
		await agent2.groupChatPage.waitForMessage('Alice break A');

		await agent2.groupChatPage.sendMessage('Bob cluster first');
		await agent2.groupChatPage.sendMessage('Bob cluster middle');
		await agent2.groupChatPage.sendMessage('Bob cluster last');
		await agent1.groupChatPage.waitForMessage('Bob cluster last');

		await agent1.groupChatPage.sendMessage('Alice break B');
		await agent2.groupChatPage.waitForMessage('Alice break B');

		await agent2.groupChatPage.sendMessage('Bob single');
		await agent1.groupChatPage.waitForMessage('Bob single');

		const firstHash =
			await agent1.groupChatPage.getMessageHash('Bob cluster first');
		const middleHash =
			await agent1.groupChatPage.getMessageHash('Bob cluster middle');
		const lastHash =
			await agent1.groupChatPage.getMessageHash('Bob cluster last');
		const singleHash = await agent1.groupChatPage.getMessageHash('Bob single');

		expect(firstHash).not.toBeNull();
		expect(middleHash).not.toBeNull();
		expect(lastHash).not.toBeNull();
		expect(singleHash).not.toBeNull();

		expect(await agent1.groupChatPage.getSenderName(firstHash!)).toBe('Bob');
		expect(await agent1.groupChatPage.getSenderName(singleHash!)).toBe('Bob');

		expect(await agent1.groupChatPage.getSenderName(middleHash!)).toBeNull();
		expect(await agent1.groupChatPage.getSenderName(lastHash!)).toBeNull();

		const firstColor =
			await agent1.groupChatPage.getSenderColorVar(firstHash!);
		const singleColor =
			await agent1.groupChatPage.getSenderColorVar(singleHash!);
		expect(firstColor).toMatch(/^--sender-color-\d+$/);
		expect(singleColor).toBe(firstColor);
	});
});
