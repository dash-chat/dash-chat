import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Full messaging flow', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
	});

	it('creates profiles on both agents', async () => {
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
	});

	it('sends a contact request from Alice to Bob', async () => {
		// One-directional on purpose: Bob does not add Alice back, so the
		// pre-accept checks below run while the request is still pending.
		await navigateToAddContact(agent1);
		await navigateToAddContact(agent2);
		const bobLink = await agent2.addContactPage.getAddContactLink();
		await agent1.addContactPage.enterAddContactLink(bobLink);
		await agent1.directChatPage.ready();
	});

	it('lets Alice send messages before Bob accepts the request', async () => {
		await agent1.directChatPage.composer.sendMessage('Hello before accept!');
		await agent1.directChatPage.messages.waitForMessage('Hello before accept!');
	});

	it('shows Alice’s messages to Bob before he accepts the request', async () => {
		await agent2.addContactPage.back.click();
		await agent2.newMessagePage.back.click();
		await agent2.homePage.openChat('Alice Test');
		// The request is still unanswered: the accept bar is showing while the
		// pre-accept message is already readable.
		await agent2.directChatPage.acceptButton.waitForExist();
		await agent2.directChatPage.messages.waitForMessage('Hello before accept!');
	});

	it('summarizes the chat as a message request while it is pending', async () => {
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();
		const rowText = await agent2.homePage.chatRowText('Alice Test');
		expect(rowText).toContain(await agent2.tr('messageRequest'));
		expect(rowText).not.toContain('Hello before accept!');
		await agent2.homePage.openChat('Alice Test');
		await agent2.directChatPage.acceptButton.waitForExist();
	});

	it('establishes the contact when Bob accepts the request', async () => {
		await agent2.directChatPage.acceptButton.click();
		await agent2.directChatPage.acceptConfirm.click();
		await agent2.directChatPage.composer.messageInput.waitForExist();
		await agent2.directChatPage.acceptButton.waitForExist({ reverse: true });
	});

	it('sends a message from Alice to Bob', async () => {
		await agent1.directChatPage.composer.sendMessage('Hello from Alice!');
		await agent1.directChatPage.messages.waitForMessage('Hello from Alice!');
		await agent2.directChatPage.messages.waitForMessage('Hello from Alice!');
	});

	it('sends a reply from Bob to Alice', async () => {
		await agent2.directChatPage.composer.sendMessage('Hello from Bob!');
		await agent2.directChatPage.messages.waitForMessage('Hello from Bob!');
		await agent1.directChatPage.messages.waitForMessage('Hello from Bob!');
	});

	it('truncates a long message and reveals it on Read more', async () => {
		const long = `${'A'.repeat(900)} TAIL_MARKER ${'B'.repeat(100)}`;
		await agent1.directChatPage.composer.sendMessage(long);
		await agent1.directChatPage.readMore.waitForExist();
		// The hidden tail is not rendered until expanded.
		expect(
			await agent1.directChatPage.messages.messageAreaContains('TAIL_MARKER'),
		).toBe(false);
		await agent1.directChatPage.readMore.click();
		await agent1.directChatPage.messages.waitForMessage('TAIL_MARKER');
	});

	it('finds a search match hidden in a truncated message tail', async () => {
		const long = `${'C'.repeat(900)} HIDDEN_NEEDLE ${'D'.repeat(100)}`;
		await agent1.directChatPage.composer.sendMessage(long);
		await agent1.directChatPage.readMore.waitForExist();
		expect(
			await agent1.directChatPage.messages.messageAreaContains('HIDDEN_NEEDLE'),
		).toBe(false);

		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();
		await agent1.chatSettingsPage.searchButton.click();
		await agent1.directChatPage.searchFor('HIDDEN_NEEDLE');

		await agent1.directChatPage.messages.waitForMessage('HIDDEN_NEEDLE');
		await expect(agent1.directChatPage.searchResultsCount).not.toHaveText(
			await agent1.tr('noResults'),
		);
	});

	it('does not match the Read more button label', async () => {
		// A truncated message renders a "Read more" button; searching for its
		// label must not produce a false match from the UI chrome.
		await agent1.directChatPage.searchFor(await agent1.tr('readMore'));
		await expect(agent1.directChatPage.searchResultsCount).toHaveText(
			await agent1.tr('noResults'),
		);
	});
});
