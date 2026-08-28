import { exchangeContactsAndCreateGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SOLID_PNG_BYTES } from '../../helpers/images';
import { type Agent, setupAgents } from '../../setup/setup-agents';

const PHOTO = {
	name: 'group-photo.png',
	mimeType: 'image/png',
	bytes: SOLID_PNG_BYTES,
};

describe('Group details spec', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await exchangeContactsAndCreateGroup(agent1, agent2);
	});

	it('Shows a new name to all members if the admin changes it', async () => {
		await agent1.groupChatPage.infoLink.click();
		await agent1.groupInfoPage.ready();
		await agent1.groupInfoPage.editLink.click();
		await agent1.groupInfoEditPage.ready();
		await agent1.groupInfoEditPage.setName('renamed group');
		await agent1.groupInfoEditPage.save();
		await agent1.groupInfoPage.ready();

		await agent2.homePage.chatListItem('renamed group').waitForExist();
		await agent2.homePage.chatListItem('renamed group').click();
		await agent2.groupChatPage.ready();
		await agent2.waitUntil(async () =>
			(await agent2.groupChatPage.headerName.getText()).includes(
				'renamed group',
			),
		);
	});

	it('Hides the edit details link from non-admins', async () => {
		await agent2.groupChatPage.infoLink.click();
		await agent2.groupInfoPage.ready();

		await expect(agent2.groupInfoPage.editLink).not.toBeDisplayed();
	});

	it('Shows "You" instead of your own profile name in the members list', async () => {
		await agent2.groupInfoPage.memberItem('Bob').waitForExist();
		expect(await agent2.groupInfoPage.memberItem('Bob').getText()).toContain(
			'You',
		);
		expect(await agent2.groupInfoPage.memberItem('Alice').getText()).toContain(
			'Alice',
		);
	});

	it('Shows a new photo and description to all members if the admin changes them', async () => {
		await agent1.groupInfoPage.editLink.click();
		await agent1.groupInfoEditPage.ready();
		await agent1.groupInfoEditPage.setDescription('A group about nothing');

		await agent1.groupInfoEditPage.editPhotoButton.click();
		await agent1.editPhotoPage.ready();
		await agent1.editPhotoPage.pickPhoto(PHOTO);
		const savedPhoto = await agent1.editPhotoPage.avatar.imageSrc();
		await agent1.editPhotoPage.save();
		await agent1.groupInfoEditPage.ready();

		await agent1.groupInfoEditPage.save();
		await agent1.groupInfoPage.ready();
		expect(await agent1.groupInfoPage.avatar.imageSrc()).toBe(savedPhoto);

		await agent2.waitUntil(async () =>
			(await agent2.groupInfoPage.description.getText()).includes(
				'A group about nothing',
			),
		);
		expect(await agent2.groupInfoPage.avatar.imageSrc()).toBe(savedPhoto);
	});
});
