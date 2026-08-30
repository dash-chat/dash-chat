import { blockAgent } from '../../helpers/flows/block-agent';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('block contact', () => {
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
		await agent1.directChatPage.waitForPeerProfile();
	});

	it('blocks from chat settings and shows the indicators', async () => {
		await agent1.directChatPage.composer.sendMessage('Hello Bob');

		await blockAgent(agent1);
		await agent1.directChatPage.blockedBanner.waitForDisplayed();
		await agent1.directChatPage.blockedNameIcon.waitForDisplayed();

		await expect(
			agent1.directChatPage.messages.systemMessage('contact_blocked'),
		).toHaveText(await agent1.tr('youBlockedContact', { name: 'Bob Test' }));
		await expect(
			agent2.directChatPage.messages.systemMessage('contact_blocked'),
		).not.toBeExisting();

		await agent2.directChatPage.composer.sendMessage('Sent while blocked');
		await agent2.directChatPage.messages.waitForMessage('Sent while blocked');
		expect(
			await agent1.directChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);

		// The chat turns read-only: no swipe, react, reply, or edit — only the
		// local actions remain. Copy doubles as the way to close the menu again.
		const target =
			await agent1.directChatPage.messages.waitForMessage('Hello Bob');
		if (agent1.platform !== 'desktop') {
			const engaged = await agent1.execute(
				(hash: string) => window.__test.swipeToReply(hash),
				target.hash,
			);
			expect(engaged).toBe(false);
		} else {
			await expect(target.hoverReplyButton).not.toBeExisting();
			await expect(target.hoverReactButton).not.toBeExisting();
		}
		await target.openActions();
		if (agent1.platform !== 'desktop') {
			await expect(target.quickReactionBar).not.toBeExisting();
		}
		await expect(target.replyAction).not.toBeExisting();
		await expect(target.editAction).not.toBeExisting();
		await expect(target.copyAction).toBeExisting();
		await target.copyAction.click();

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.blockedRowIcon.waitForDisplayed();
	});

	it('unblocks from the blocked banner', async () => {
		await agent1.homePage.chatRow.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.unblockButton.click();
		await agent1.directChatPage.unblockConfirm.waitForClickable();
		await agent1.directChatPage.unblockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed({
			reverse: true,
		});

		await expect(
			agent1.directChatPage.messages.systemMessage('contact_unblocked'),
		).toHaveText(await agent1.tr('youUnblockedContact', { name: 'Bob Test' }));
		// The earlier block stays in the timeline — it's history, not state.
		await expect(
			agent1.directChatPage.messages.systemMessage('contact_blocked'),
		).toBeExisting();

		// Unblocking makes the chat writable again, so the actions return.
		const target =
			await agent1.directChatPage.messages.waitForMessage('Hello Bob');
		await target.openActions();
		await expect(target.replyAction).toBeExisting();
		await expect(target.editAction).toBeExisting();
		await target.copyAction.click();

		// Bob's log is sequential, so once this later message arrives the one
		// sent while blocked has also been synced — and rejected — by Alice.
		await agent2.directChatPage.composer.sendMessage('After unblock');
		await agent1.directChatPage.messages.waitForMessage('After unblock');
		expect(
			await agent1.directChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);

		// A cold start rebuilds the message list from the raw op store, which
		// holds the blocked-time op the backend rejected — the message must
		// stay hidden on this path too, even now that Bob is unblocked.
		await agent1.restart();
		await agent1.homePage.ready();
		await agent1.homePage.chatRow.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.messages.waitForMessage('After unblock');
		expect(
			await agent1.directChatPage.messages.messageAreaContains(
				'Sent while blocked',
			),
		).toBe(false);
	});

	it('blocks from the new-message contact menu', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();

		await agent1.newMessagePage.openContactMenu('Bob');
		await agent1.newMessagePage.contactActionsMenu.block.click();
		await agent1.newMessagePage.contactActionsMenu.blockConfirm.waitForClickable();
		await agent1.newMessagePage.contactActionsMenu.blockConfirm.click();

		await agent1.toast.expectMessage(
			await agent1.tr('contactBlockedToast', { name: 'Bob Test' }),
		);
		await expect(agent1.newMessagePage.contactItem('Bob')).not.toBeExisting();
	});

	it('hides blocked contacts from the group member pickers', async () => {
		await agent1.newMessagePage.newGroup.click();
		await agent1.newGroupPage.addMembersStep.ready();

		const noContacts = await agent1.tr('noContactsYet');
		await expect(
			agent1.newGroupPage.addMembersStep.contactList.emptyMessage,
		).toHaveText(noContacts);
		await expect(
			agent1.newGroupPage.addMembersStep.contactList.contactItem('Bob'),
		).not.toBeExisting();

		await agent1.newGroupPage.addMembersStep.nextButton.click();
		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('Blocked Picker Group');
		await agent1.newGroupPage.groupInfoStep.createButton.click();
		await agent1.groupChatPage.ready();

		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.addMembersLink.click();
		await agent1.addMembersPage.ready();

		await expect(agent1.addMembersPage.contactList.emptyMessage).toHaveText(
			noContacts,
		);
		await expect(
			agent1.addMembersPage.contactList.contactItem('Bob'),
		).not.toBeExisting();
	});
});
