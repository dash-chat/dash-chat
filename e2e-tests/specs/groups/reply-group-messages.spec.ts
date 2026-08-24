import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Replying to group messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
		await agent1.groupChatPage.ready();

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('replies to a member message and renders the quote on both sides', async () => {
		await agent1.groupChatPage.composer.sendMessage('Who is bringing snacks?');
		const target = await agent2.groupChatPage.messages.waitForMessage(
			'Who is bringing snacks?',
		);

		await target.reply('I am');

		const reply2 = await agent2.groupChatPage.messages.waitForMessage('I am');
		await reply2.waitForReplyQuote('Who is bringing snacks?');
		const reply1 = await agent1.groupChatPage.messages.waitForMessage('I am');
		await reply1.waitForReplyQuote('Who is bringing snacks?');
	});

	it('scrolls to the replied-to message when the quote is clicked', async () => {
		const reply = await agent1.groupChatPage.messages.waitForMessage('I am');
		await reply.clickReplyQuote();

		const target = await agent1.groupChatPage.messages.waitForMessage(
			'Who is bringing snacks?',
		);
		await agent1.waitUntil(() => target.isFlashed(), {
			timeoutMsg: 'Original message was not highlighted after quote click',
		});
	});
});
