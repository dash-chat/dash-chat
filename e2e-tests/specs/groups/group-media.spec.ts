/**
 * Group media E2E — verifies that photo and file attachments work in group
 * chats the same way they do in direct chats.
 */

import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../../setup/setup-agents';

describe('Group media attachments', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Media');
		await agent2.createProfilePage.createProfile('Bob', 'Media');
		await exchangeContacts(agent1, agent2);
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.addContactByName('Bob');
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName('mediagroup');
		await agent1.newGroupPage.groupInfoStep.createButton.click();
		await agent1.groupChatPage.ready();

		await agent2.homePage.chatListItem('mediagroup').waitForExist();
		await agent2.homePage.chatListItem('mediagroup').click();
		await agent2.groupChatPage.ready();
	});

	it('sends photos with a caption to the group', async () => {
		await agent1.groupChatPage.composer.attachPhotos(2);
		await agent1.groupChatPage.composer.expectStagedPhotoCount(2);
		await agent1.groupChatPage.sendMessage('group pics');
		await agent1.groupChatPage.waitForPhotoMessage();
		await agent2.groupChatPage.waitForMessage('group pics');
		await agent2.groupChatPage.waitForPhotoMessage();
	});

	it('sends a file attachment to the group', async () => {
		await agent2.groupChatPage.composer.attachFile(
			'group-notes.txt',
			'notes for the group',
			'text/plain',
		);
		await agent2.groupChatPage.composer.send();
		await agent2.groupChatPage.waitForFileMessage('group-notes.txt');
		await agent1.groupChatPage.waitForFileMessage('group-notes.txt');
	});
});
