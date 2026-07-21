/**
 * Long-name truncation E2E test.
 *
 * Verifies that contacts with very long names don't cause horizontal overflow
 * in the chat list, the direct-chat navbar, the contact-request banner, the
 * in-chat peer-name welcome banner, the chat-settings page, the peer profile
 * sheet, and the profile-settings list item.
 */
import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

const LONG_NAME = 'Bartholomew';
const LONG_SURNAME = 'Wolfeschlegelsteinhausenbergerdorff';

describe('Long name truncation', () => {
	let agent1: Agent;
	let agent2: Agent;
	let agent1Code = '';
	let agent2Code = '';

	before(async function () {
		[agent1, agent2] = await setupAgents(this, ['any', 'any']);
	});

	it('creates profiles — agent1 with a very long name', async () => {
		await agent1.createProfilePage.createProfile(LONG_NAME, LONG_SURNAME);
		await agent2.createProfilePage.createProfile('Bob', 'Test');
	});

	// One-way exchange: agent2 sees a pending request from agent1 — lets us
	// assert overflow on the pending chat-list entry and the contact-request
	// banner before agent2 accepts.
	it('agent1 sends a one-way contact request — chat list has no overflow', async () => {
		await navigateToAddContact(agent1);
		await navigateToAddContact(agent2);
		agent1Code = await agent1.addContactPage.getAddContactLink();
		agent2Code = await agent2.addContactPage.getAddContactLink();
		await agent1.addContactPage.enterAddContactLink(agent2Code);

		await agent2.addContactPage.back.click();
		await agent2.newMessagePage.back.click();
		await agent2.homePage.ready();
		await agent2.waitUntil(
			async () => agent2.homePage.hasChatListItem(LONG_NAME),
			{ timeout: 30_000 },
		);
		expect(await agent2.homePage.checkChatListOverflow()).toEqual([]);
	});

	it('agent2 opens the pending chat — contact-request banner has no overflow', async () => {
		await agent2.homePage.openChat(LONG_NAME);
		await agent2.waitUntil(async () =>
			agent2.directChatPage.isContactRequestBannerVisible(),
		);
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('agent2 accepts the contact — navbar and welcome banner have no overflow', async () => {
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();
		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterAddContactLink(agent1Code);
		await agent2.directChatPage.ready();

		expect(await agent2.directChatPage.isPeerNamePresent()).toBe(true);
		expect(await agent2.directChatPage.checkNavbarOverflow()).toEqual([]);
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('opens chat-settings — chat-settings page has no overflow', async () => {
		await agent2.directChatPage.settingsLink.click();
		await agent2.chatSettingsPage.ready();
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it('opens peer profile sheet — sheet has no overflow', async () => {
		await agent2.chatSettingsPage.peerName.click();
		await agent2.waitUntil(async () => agent2.peerProfileSheet.isOpen());
		expect(await agent2.checkOverflow()).toEqual([]);
	});

	it("opens agent1's settings/profile — name list item has no overflow", async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.settingsLink.click();
		await agent1.settingsPage.ready();
		await agent1.settingsPage.profileLink.click();
		await agent1.profilePage.ready();
		await agent1.waitUntil(async () =>
			agent1.profilePage.nameItemContains(LONG_NAME),
		);
		expect(await agent1.checkOverflow()).toEqual([]);
	});
});
