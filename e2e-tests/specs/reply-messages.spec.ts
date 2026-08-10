import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Replying to messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('replies to a peer message and renders the quote on both sides', async () => {
		await agent1.directChatPage.composer.sendMessage('Shall we meet tomorrow?');
		const target = await agent2.directChatPage.messages.waitForMessage(
			'Shall we meet tomorrow?',
		);

		await target.reply('Sure, at noon');

		const reply2 =
			await agent2.directChatPage.messages.waitForMessage('Sure, at noon');
		await reply2.waitForReplyQuote('Shall we meet tomorrow?');
		const reply1 =
			await agent1.directChatPage.messages.waitForMessage('Sure, at noon');
		await reply1.waitForReplyQuote('Shall we meet tomorrow?');
	});

	it('scrolls to the replied-to message when the quote is clicked', async () => {
		const reply =
			await agent2.directChatPage.messages.waitForMessage('Sure, at noon');
		await reply.clickReplyQuote();

		const target = await agent2.directChatPage.messages.waitForMessage(
			'Shall we meet tomorrow?',
		);
		await agent2.waitUntil(() => target.isFlashed(), {
			timeoutMsg: 'Original message was not highlighted after quote click',
		});
	});

	it('keeps the composer pinned to the bottom when the quote jump cannot scroll further', async () => {
		if (!(await agent2.supportsWideScreen())) return;
		await agent2.setWideScreen(true);
		try {
			const reply =
				await agent2.directChatPage.messages.waitForMessage('Sure, at noon');
			await reply.clickReplyQuote();
			// The jump animates, and so does the shell scroll this guards
			// against — sample once it has settled, not mid-glide.
			await agent2.pause(1_000);
			expect(await agent2.directChatPage.composer.bottomGap()).toBe(0);
		} finally {
			await agent2.setWideScreen(false);
		}
	});

	it('keeps the quoted content frozen when the target is edited, and still scrolls to it', async () => {
		const original = await agent1.directChatPage.messages.waitForMessage(
			'Shall we meet tomorrow?',
		);
		await original.edit('Shall we meet tomorrow?', 'Shall we meet on Friday?');
		await agent1.directChatPage.messages.waitForMessage(
			'Shall we meet on Friday?',
		);
		const edited = await agent2.directChatPage.messages.waitForMessage(
			'Shall we meet on Friday?',
		);

		// The quote still shows the version that was replied to...
		const reply =
			await agent2.directChatPage.messages.waitForMessage('Sure, at noon');
		await reply.waitForReplyQuote('Shall we meet tomorrow?');

		// ...while clicking it scrolls to the edited message's position.
		await reply.clickReplyQuote();
		await agent2.waitUntil(() => edited.isFlashed(), {
			timeoutMsg: 'Edited message was not highlighted after quote click',
		});
	});

	it('replaces the quoted content with a tombstone when the target is deleted for everyone', async () => {
		await agent1.directChatPage.composer.sendMessage('Secret plans');
		const target =
			await agent2.directChatPage.messages.waitForMessage('Secret plans');
		await target.reply('Acknowledged');

		const reply1 =
			await agent1.directChatPage.messages.waitForMessage('Acknowledged');
		await reply1.waitForReplyQuote('Secret plans');

		const mine =
			await agent1.directChatPage.messages.waitForMessage('Secret plans');
		await mine.deleteForEveryone();

		// The deleted content never shows anywhere — including inside quotes.
		const placeholder = await agent2.tr('thisMessageWasDeleted');
		await agent1.directChatPage.messages.waitForMessageGone('Secret plans');
		await agent2.directChatPage.messages.waitForDeleted(
			'Secret plans',
			placeholder,
		);

		await agent1.waitUntil(() => reply1.replyQuoteIsDeleted(), {
			timeoutMsg: 'No tombstone quote on the author side',
		});
		const reply2 =
			await agent2.directChatPage.messages.waitForMessage('Acknowledged');
		await agent2.waitUntil(() => reply2.replyQuoteIsDeleted(), {
			timeoutMsg: 'No tombstone quote on the peer side',
		});

		// Clicking the tombstone quote still scrolls to the deleted placeholder.
		await reply2.clickReplyQuote();
		const deleted =
			await agent2.directChatPage.messages.waitForMessage(placeholder);
		await agent2.waitUntil(() => deleted.isFlashed(), {
			timeoutMsg: 'Deleted placeholder was not highlighted',
		});
	});

	it('shows the tombstone with a warning when the target was deleted for me, without scrolling', async () => {
		await agent1.directChatPage.composer.sendMessage('Ephemeral note');
		const target =
			await agent2.directChatPage.messages.waitForMessage('Ephemeral note');
		await target.reply('Noted, thanks');

		const reply2 =
			await agent2.directChatPage.messages.waitForMessage('Noted, thanks');
		await reply2.waitForReplyQuote('Ephemeral note');

		await target.deleteForMe();
		await agent2.directChatPage.messages.waitForMessageGone('Ephemeral note');

		await agent2.waitUntil(() => reply2.replyQuoteIsDeletedForMe(), {
			timeoutMsg: 'No deleted-for-me warning on the quote',
		});

		// The other side is unaffected and still sees the quoted content.
		const reply1 =
			await agent1.directChatPage.messages.waitForMessage('Noted, thanks');
		await reply1.waitForReplyQuote('Ephemeral note');
	});
});
