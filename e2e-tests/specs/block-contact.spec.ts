import { blockAgent } from '../helpers/flows/block-agent';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

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
		await agent1.directChatPage.peerName.waitForDisplayed();
	});

	it('blocks from chat settings and shows the indicators', async () => {
		await blockAgent(agent1);
		await agent1.directChatPage.blockedBanner.waitForDisplayed();
		await agent1.directChatPage.blockedNameIcon.waitForDisplayed();

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
