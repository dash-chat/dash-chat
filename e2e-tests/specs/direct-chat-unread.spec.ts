/**
 * Direct chat unread messages E2E tests.
 *
 * Verifies that messages received while away from the chat show the
 * chat-row unread badge, and that simply entering the chat (without
 * scrolling) marks the visible messages as read.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Direct chat unread messages', () => {
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

	it('shows the chat-row unread badge for messages received while at home', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();

		for (let i = 1; i <= 3; i++) {
			await agent2.directChatPage.composer.sendMessage(`while away ${i}`);
		}

		await agent1.homePage.unreadBadge.waitForExist({ timeout: SYNC_TIMEOUT });
		await expect(agent1.homePage.unreadBadge).toHaveText('3');
	});

	it('marks the messages read on entering the chat, without scrolling', async () => {
		await agent1.homePage.openChat('Bob');
		await agent1.directChatPage.messages.waitForMessage('while away 3');
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(true);

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.unreadBadge.waitForDisplayed({
			reverse: true,
			timeoutMsg:
				'Unread badge did not clear after entering the chat without scrolling',
		});
	});

	it('clears the badge on entering even when the unread messages overflow the viewport', async () => {
		const COUNT = 25;
		for (let i = 1; i <= COUNT; i++) {
			await agent2.directChatPage.composer.sendMessage(`overflow ${i}`);
		}

		await agent1.homePage.unreadBadge.waitForExist({ timeout: SYNC_TIMEOUT });
		await expect(agent1.homePage.unreadBadge).toHaveText(`${COUNT}`);

		await agent1.homePage.openChat('Bob');
		await agent1.directChatPage.messages.waitForMessage(`overflow ${COUNT}`);
		expect(await agent1.directChatPage.scroll.isAtBottom()).toBe(true);

		await agent1.directChatPage.messages.unreadDivider.waitForExist({
			timeoutMsg:
				'Unread divider did not show on entering a chat with unread messages',
		});
		expect(await agent1.directChatPage.messages.unreadDividerText()).toContain(
			`${COUNT}`,
		);

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.unreadBadge.waitForDisplayed({
			reverse: true,
			timeoutMsg:
				'Unread badge did not clear after entering a chat whose unread messages overflow the viewport',
		});
	});
});
