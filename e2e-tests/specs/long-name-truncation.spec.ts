/**
 * Long-name truncation E2E test.
 *
 * Verifies that contacts with very long names don't cause horizontal overflow
 * in the chat list, the direct-chat navbar, the contact-request banner, the
 * in-chat peer-name welcome banner, the chat-settings page, the peer profile
 * sheet, and the profile-settings list item.
 */
import { S } from '../../ui/tests/selectors';
import { type Agent, setupAgent } from '../helpers/setup-agents';

const LONG_NAME = 'Bartholomew';
const LONG_SURNAME = 'Wolfeschlegelsteinhausenbergerdorff';

describe('Long name truncation', () => {
	let agent1: Agent;
	let agent2: Agent;
	let agent1Code = '';
	let agent2Code = '';

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

	// One-way exchange: agent2 sees a pending request from agent1 — lets us
	// assert overflow on the pending chat-list entry and the contact-request
	// banner before agent2 accepts.
	it('agent1 sends a one-way contact request — chat list has no overflow', async () => {
		await agent1.navigateToAddContact();
		await agent2.navigateToAddContact();
		const code1 = await agent1.getContactCode();
		const code2 = await agent2.getContactCode();
		if (!code1 || !code2) throw new Error('contact code missing');
		agent1Code = code1;
		agent2Code = code2;
		await agent1.addContact(agent2Code);

		await agent2.goto('/');
		await agent2.waitUntil(async () => agent2.hasChatListItem(LONG_NAME), {
			timeout: 30_000,
			interval: 1_000,
			timeoutMsg: 'Long-name contact not in chat list',
		});
		expect(await agent2.checkChatListOverflow()).toEqual([]);
	});

	it('agent2 opens the pending chat — contact-request banner has no overflow', async () => {
		await agent2.openDirectChat(LONG_NAME);
		await agent2.waitUntil(
			async () => agent2.isContactRequestBannerVisible(),
			{
				timeout: 10_000,
				interval: 500,
				timeoutMsg: 'Contact-request banner not rendered',
			},
		);
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('agent2 accepts the contact — navbar and welcome banner have no overflow', async () => {
		await agent2.goto('/');
		await agent2.navigateToAddContact();
		await agent2.addContact(agent1Code);

		expect(await agent2.isPeerNamePresent()).toBe(true);
		expect(await agent2.checkNavbarOverflow()).toEqual([]);
		// The welcome area (avatar + full name above messages) is part of the
		// direct-chat page — full-page overflow scan covers it.
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('opens chat-settings — chat-settings page has no overflow', async () => {
		await agent2.click(S.directChat.settingsLink);
		await agent2.waitUntil(async () => agent2.chatSettingsLoaded(), {
			timeout: 10_000,
			interval: 500,
			timeoutMsg: 'Chat-settings page did not load',
		});
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('opens peer profile sheet — sheet has no overflow', async () => {
		await agent2.click(S.chatSettings.peerName);
		await agent2.waitUntil(async () => agent2.isPeerProfileSheetOpen(), {
			timeout: 5_000,
			interval: 250,
			timeoutMsg: 'Peer profile sheet did not open',
		});
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('opens agent1\'s settings/profile — name list item has no overflow', async () => {
		// agent1 owns the long name — view their own profile-settings list item.
		await agent1.goto('/settings/profile');
		await agent1.waitUntil(
			async () => agent1.profileNameListItemContains(LONG_NAME),
			{
				timeout: 10_000,
				interval: 500,
				timeoutMsg: 'Profile-settings list item not visible',
			},
		);
		expect(await agent1.checkOverflow()).toEqual([]);
	});
});
