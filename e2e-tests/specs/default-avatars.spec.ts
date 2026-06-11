import { avatarAppearance } from '../helpers/components/avatar';
import { tid } from '../helpers/selectors';
import { type Agent, setupAgent } from '../setup/setup-agents';

const TEXT_AVATAR_TEXT_COLOR = 'rgb(131, 24, 67)';

describe('Default avatars', () => {
	let agent: Agent;

	before(async () => {
		agent = await setupAgent('agent1');
		// Lower-cased on purpose: the initials must come out upper-cased.
		await agent.createProfilePage.createProfile('avatar', 'tester');
		await agent.homePage.ready();
	});

	it('renders a profile without a photo as initials on an assigned color', async () => {
		const avatar = await avatarAppearance(agent, tid('home-settings-link'));
		expect(avatar.initials).toBe('AT');
		expect(avatar.color).toBe(TEXT_AVATAR_TEXT_COLOR);
	});

	it('prefills the text-avatar editor with the same initials and color', async () => {
		const homeAvatar = await avatarAppearance(agent, tid('home-settings-link'));

		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
		await agent.profilePage.editPhoto.click();
		await agent.editPhotoPage.ready();
		await agent.editPhotoPage.textButton.click();
		await agent.editPhotoPage.textPreview.waitForExist();

		const editor = await agent.editPhotoPage.textAvatarState();
		expect(editor.text).toBe('AT');
		expect(editor.backgroundColor).toBe(homeAvatar.backgroundColor);
	});

	it('stores nothing when the prefilled editor is dismissed', async () => {
		await agent.editPhotoPage.back.click();
		await agent.editPhotoPage.ready();
		expect(await agent.editPhotoPage.saveButton.isEnabled()).toBe(false);

		await agent.editPhotoPage.close.click();
		await agent.profilePage.ready();
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();

		const avatar = await avatarAppearance(agent, tid('settings-profile-link'));
		expect(avatar.initials).toBe('AT');
	});
});
