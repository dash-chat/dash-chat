import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Editing messages', () => {
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

	it('edits a message in place and shows the "Edited" indicator on both sides', async () => {
		await agent1.directChatPage.composer.sendMessage('Helo world');
		const message =
			await agent1.directChatPage.messages.waitForMessage('Helo world');
		await agent2.directChatPage.messages.waitForMessage('Helo world');

		await message.edit('Helo world', 'Hello world');

		// Author and peer both converge on the corrected text in place.
		const edited1 =
			await agent1.directChatPage.messages.waitForMessage('Hello world');
		const edited2 =
			await agent2.directChatPage.messages.waitForMessage('Hello world');

		await browser.waitUntil(() => edited1.hasEditedIndicator(), {
			timeoutMsg: 'No "Edited" indicator on the author side',
		});
		await browser.waitUntil(() => edited2.hasEditedIndicator(), {
			timeoutMsg: 'No "Edited" indicator on the peer side',
		});
	});

	it('does not offer Edit on the peer’s messages', async () => {
		await agent2.directChatPage.composer.sendMessage("Bob's message");
		const message =
			await agent1.directChatPage.messages.waitForMessage("Bob's message");

		await message.openActions();
		expect(await message.editAction.isExisting()).toBe(false);
	});

	it('copies a message to the clipboard from the actions menu', async () => {
		const message =
			await agent1.directChatPage.messages.waitForMessage("Bob's message");
		await message.openActions();
		await message.copyAction.waitForClickable();
		await message.copyAction.click();
		await agent1.toast.expectMessage(
			await agent1.tr('copiedMessageToClipboard'),
		);
	});

	it('asks before discarding a draft when starting an edit', async () => {
		const { composer, messages } = agent1.directChatPage;
		const message = await messages.waitForMessage('Hello world');

		await composer.type('Draft in progress');
		await message.openActions();
		await message.editAction.waitForClickable();
		await message.editAction.click();
		await composer.discardDraftConfirm.waitForClickable();

		// Cancel keeps the draft and stays out of edit mode.
		await composer.discardDraftCancel.click();
		await composer.discardDraftConfirm.waitForClickable({ reverse: true });
		expect(await composer.editingBanner.isExisting()).toBe(false);
		expect(await composer.messageInput.getValue()).toBe('Draft in progress');

		// Discard drops the draft and enters edit mode prefilled.
		await message.openActions();
		await message.editAction.waitForClickable();
		await message.editAction.click();
		await composer.discardDraftConfirm.waitForClickable();
		await composer.discardDraftConfirm.click();
		await composer.editingBanner.waitForExist();
		await browser.waitUntil(
			async () => (await composer.messageInput.getValue()) === 'Hello world',
			{ timeoutMsg: 'Editing input is not prefilled with the message text' },
		);

		await composer.cancelEditButton.click();
		await composer.editingBanner.waitForExist({ reverse: true });
	});

	// Desktop offers two ways into the actions menu: the hover toolbar's ⋯
	// button (covered above) and a right-click on the bubble, which opens
	// MessageContextMenu at the cursor instead.
	it('opens a working actions menu on right-click', async function () {
		if (agent1.platform !== 'desktop') this.skip();
		const message =
			await agent1.directChatPage.messages.waitForMessage('Hello world');

		await message.openActionsByRightClick();
		// Clickability, not mere presence: the menu fades in, and a closed
		// popover still counts as displayed while its opacity is rounding to 0.
		await message.contextMenuCopyAction.waitForClickable();
		await message.contextMenuCopyAction.click();
		await agent1.toast.expectMessage(
			await agent1.tr('copiedMessageToClipboard'),
		);
	});
});
