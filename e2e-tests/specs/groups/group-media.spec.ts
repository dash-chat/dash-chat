/**
 * Group media E2E — verifies that photo and file attachments work in group
 * chats the same way they do in direct chats.
 */
import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Group media attachments', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		({ agent1, agent2 } = await setupAgents(this, {
			agent1: 'any',
			agent2: 'any',
		}));
		await exchangeContactsAndCreateGroup(agent1, agent2);

		// The flow leaves agent2 on the home page; open the group so it can
		// receive the media sent below.
		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();
	});

	it('sends photos with a caption to the group', async () => {
		await agent1.groupChatPage.composer.attachPhotos('group');
		await agent1.groupChatPage.composer.attachPhotos('group');
		await agent1.groupChatPage.composer.expectStagedPhotoCount(2);
		await agent1.groupChatPage.sendMessage('group pics');
		await agent1.groupChatPage.messages.waitForPhotoMessage('group');
		await agent2.groupChatPage.messages.waitForMessage('group pics');
		await agent2.groupChatPage.messages.waitForPhotoMessage('group');
	});

	it('sends a file attachment to the group', async () => {
		await agent2.groupChatPage.composer.attachFile(
			'group-notes.txt',
			'notes for the group',
			'text/plain',
		);
		await agent2.groupChatPage.composer.send();
		await agent2.groupChatPage.messages.waitForFileMessage('group-notes.txt');
		await agent1.groupChatPage.messages.waitForFileMessage('group-notes.txt');
	});
});
