import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { tid } from '../../helpers/selectors';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Leaving group', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);

		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();
	});

	it('creator can leave a group they created alone', async () => {
		await createGroup(agent1, 'Solo Group', null);

		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.composer.sendMessage('Hello group');
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();

		await agent1.groupInfoPage.leaveButton.click();
		await agent1.groupInfoPage.leaveConfirmButton.waitForExist();
		await agent1.groupInfoPage.leaveConfirmButton.click();

		await agent1.homePage.ready();

		// Group remains in chat list
		await expect(agent1.homePage.chatListItem('Solo Group')).toBeExisting();

		// Navigate back into the group
		await agent1.homePage.chatListItem('Solo Group').click();
		await agent1.groupChatPage.ready();

		// Composer is replaced by a notice (no longer a member)
		await expect(agent1.groupChatPage.notMemberNotice).toBeExisting();
		await expect(agent1.groupChatPage.composer.messageInput).not.toBeExisting();

		// System message records the departure
		const systemMessage = agent1.groupChatPage.messages.systemMessage(
			'group_member_removed',
		);
		await expect(systemMessage).toBeExisting();
		const expectedText = await agent1.tr('youLeftTheGroup');
		await expect(systemMessage).toHaveText(expectedText);

		// Leave button is gone — already left, and Alice no longer in members list
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await expect(agent1.groupInfoPage.leaveButton).not.toBeDisplayed();
		const membersList = agent1.$(tid('group-info-members'));
		await expect(membersList.$('=Alice')).not.toBeExisting();

		await agent1.groupInfoPage.back.click();
		await agent1.groupChatPage.ready();
		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
	});

	it('no longer offers reply, edit, or reactions after leaving', async function () {
		await agent1.homePage.chatListItem('Solo Group').click();
		await agent1.groupChatPage.ready();

		const target =
			await agent1.groupChatPage.messages.waitForMessage('Hello group');

		// SwipeToReply only renders on mobile; the hover toolbar only on desktop.
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

		// The actions menu keeps only the local actions — no reply or edit, and
		// the mobile spotlight carries no reaction bar. Copy doubles as the way
		// to close the menu again.
		await target.openActions();
		if (agent1.platform !== 'desktop') {
			await expect(target.quickReactionBar).not.toBeExisting();
		}
		await expect(target.replyAction).not.toBeExisting();
		await expect(target.editAction).not.toBeExisting();
		await expect(target.copyAction).toBeExisting();
		await target.copyAction.click();

		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
	});

	it('creator cant leave a group with another member but no other admins', async () => {
		await createGroup(agent1, 'Two member group', 'Bob');

		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();

		await agent1.groupInfoPage.leaveButton.click();
		await agent1.groupInfoPage.leaveConfirmButton.waitForExist();
		await agent1.groupInfoPage.leaveConfirmButton.click();

		const expectedText = await agent1.tr('errorLeavingGroupOnlyAdmin');
		await agent1.toast.expectMessage(expectedText);

		// Confirm we are still on the group info page (leave was blocked)
		await agent1.groupInfoPage.ready();
	});
});
