import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Replying to messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('replies to a peer message and renders the quote on both sides', async () => {
		await agent1.directChatPage.sendMessage('Shall we meet tomorrow?');
		await agent2.directChatPage.messages.waitForMessage(
			'Shall we meet tomorrow?',
		);

		await agent2.directChatPage.messages.replyToMessage(
			'Shall we meet tomorrow?',
			'Sure, at noon',
		);

		await agent2.directChatPage.messages.waitForReplyQuote(
			'Sure, at noon',
			'Shall we meet tomorrow?',
		);
		await agent1.directChatPage.messages.waitForReplyQuote(
			'Sure, at noon',
			'Shall we meet tomorrow?',
		);
	});

	it('scrolls to the replied-to message when the quote is clicked', async () => {
		await agent2.directChatPage.messages.clickReplyQuote('Sure, at noon');
		await agent2.waitUntil(
			() => agent2.directChatPage.messages.isFlashed('Shall we meet tomorrow?'),
			{ timeoutMsg: 'Original message was not highlighted after quote click' },
		);
	});

	it('keeps the quoted content frozen when the target is edited, and still scrolls to it', async () => {
		await agent1.directChatPage.messages.editMessage(
			'Shall we meet tomorrow?',
			'Shall we meet on Friday?',
		);
		await agent1.directChatPage.messages.waitForMessage(
			'Shall we meet on Friday?',
		);
		await agent2.directChatPage.messages.waitForMessage(
			'Shall we meet on Friday?',
		);

		// The quote still shows the version that was replied to...
		await agent2.directChatPage.messages.waitForReplyQuote(
			'Sure, at noon',
			'Shall we meet tomorrow?',
		);
		// ...while clicking it scrolls to the edited message's position.
		await agent2.directChatPage.messages.clickReplyQuote('Sure, at noon');
		await agent2.waitUntil(
			() =>
				agent2.directChatPage.messages.isFlashed('Shall we meet on Friday?'),
			{ timeoutMsg: 'Edited message was not highlighted after quote click' },
		);
	});

	it('replaces the quoted content with a tombstone when the target is deleted for everyone', async () => {
		await agent1.directChatPage.sendMessage('Secret plans');
		await agent2.directChatPage.messages.waitForMessage('Secret plans');
		await agent2.directChatPage.messages.replyToMessage(
			'Secret plans',
			'Acknowledged',
		);
		await agent1.directChatPage.messages.waitForReplyQuote(
			'Acknowledged',
			'Secret plans',
		);

		await agent1.directChatPage.messages.deleteMessage('Secret plans');

		// The deleted content never shows anywhere — including inside quotes.
		await agent1.directChatPage.messages.waitForMessageGone('Secret plans');
		await agent2.directChatPage.messages.waitForMessageGone('Secret plans');
		await agent1.waitUntil(
			() => agent1.directChatPage.messages.replyQuoteIsDeleted('Acknowledged'),
			{ timeoutMsg: 'No tombstone quote on the author side' },
		);
		await agent2.waitUntil(
			() => agent2.directChatPage.messages.replyQuoteIsDeleted('Acknowledged'),
			{ timeoutMsg: 'No tombstone quote on the peer side' },
		);

		// Clicking the tombstone quote still scrolls to the deleted placeholder.
		await agent2.directChatPage.messages.clickReplyQuote('Acknowledged');
		await agent2.waitUntil(
			() =>
				agent2.directChatPage.messages.isFlashed('This message was deleted'),
			{ timeoutMsg: 'Deleted placeholder was not highlighted' },
		);
	});

	it('shows the tombstone with a warning when the target was deleted for me, without scrolling', async () => {
		await agent1.directChatPage.sendMessage('Ephemeral note');
		await agent2.directChatPage.messages.waitForMessage('Ephemeral note');
		await agent2.directChatPage.messages.replyToMessage(
			'Ephemeral note',
			'Noted, thanks',
		);
		await agent2.directChatPage.messages.waitForReplyQuote(
			'Noted, thanks',
			'Ephemeral note',
		);

		await agent2.directChatPage.messages.deleteMessageForMe('Ephemeral note');

		await agent2.directChatPage.messages.waitForMessageGone('Ephemeral note');
		await agent2.waitUntil(
			() =>
				agent2.directChatPage.messages.replyQuoteIsDeletedForMe(
					'Noted, thanks',
				),
			{ timeoutMsg: 'No deleted-for-me warning on the quote' },
		);

		// The other side is unaffected and still sees the quoted content.
		await agent1.directChatPage.messages.waitForReplyQuote(
			'Noted, thanks',
			'Ephemeral note',
		);
	});
});
