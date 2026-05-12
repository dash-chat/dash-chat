/**
 * Long-name truncation E2E test.
 *
 * Verifies that contacts with very long names don't cause horizontal overflow
 * in the chat list or the direct-chat navbar.
 */
import { S } from '../../ui/tests/selectors';
import {
	type Agent,
	exchangeContacts,
	setupAgent,
} from '../helpers/setup-agents';

const LONG_NAME = 'Bartholomew';
const LONG_SURNAME = 'Wolfeschlegelsteinhausenbergerdorff';

describe('Long name truncation', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('creates profiles — agent1 with a very long name', async () => {
		await agent1.createProfile(LONG_NAME, LONG_SURNAME);
		await agent2.createProfile('Bob', 'Test');
	});

	it('exchanges contacts', async () => {
		await exchangeContacts(agent1, agent2);
	});

	it('chat list shows long name without horizontal overflow', async () => {
		// Wait for the chat entry to appear in agent2's chat list
		await agent2.waitUntil(
			async () =>
				agent2.execute(
					(name: string, chatListSelector: string) =>
						!!document
							.querySelector(chatListSelector)
							?.textContent?.includes(name),
					LONG_NAME,
					S.home.chatList,
				),
			{
				timeout: 30_000,
				interval: 1_000,
				timeoutMsg: 'Long-name contact not in chat list',
			},
		);

		const overflowIssues = await agent2.checkChatListOverflow();
		expect(overflowIssues).toEqual([]);
	});

	it('direct-chat navbar shows long name without horizontal overflow', async () => {
		await agent2.openDirectChat(LONG_NAME);

		// Peer name element should be present
		expect(await agent2.isPeerNamePresent()).toBe(true);

		// Navbar should not overflow
		const navbarOverflow = await agent2.checkNavbarOverflow();
		expect(navbarOverflow).toEqual([]);
	});
});
