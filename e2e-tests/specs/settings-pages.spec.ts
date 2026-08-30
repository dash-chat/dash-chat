import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Settings pages', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		await agent.createProfilePage.createProfile('Settings', 'Test');
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
	});

	it('opens the profile page', async () => {
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
		expect(await agent.profilePage.nameItemContains('Settings')).toBe(true);
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the edit-name page from profile', async () => {
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
		await agent.profilePage.editName.click();
		await agent.editNamePage.ready();
		await agent.editNamePage.back.click();
		await agent.profilePage.ready();
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the edit-about page from profile', async () => {
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
		await agent.profilePage.editAbout.click();
		await agent.editAboutPage.ready();
		await agent.editAboutPage.back.click();
		await agent.profilePage.ready();
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the edit-photo page from profile', async () => {
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
		await agent.profilePage.editPhoto.click();
		await agent.editPhotoPage.ready();
		await agent.editPhotoPage.close.click();
		await agent.profilePage.ready();
		await agent.profilePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the appearance page', async () => {
		await agent.settingsPage.appearanceLink.click();
		await agent.appearancePage.ready();
		await agent.appearancePage.back.click();
		await agent.settingsPage.ready();
	});

	it('applies the colour scheme picked on the appearance page', async () => {
		await agent.settingsPage.appearanceLink.click();
		await agent.appearancePage.ready();

		await agent.appearancePage.dark.click();
		await agent.waitUntil(
			async () => (await agent.getColorScheme()) === 'dark',
		);

		await agent.appearancePage.light.click();
		await agent.waitUntil(
			async () => (await agent.getColorScheme()) === 'light',
		);

		await agent.appearancePage.system.click();
		await agent.appearancePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the account page', async () => {
		await agent.settingsPage.accountLink.click();
		await agent.accountPage.ready();
		await agent.accountPage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the notifications page', async () => {
		await agent.settingsPage.notificationsLink.click();
		await agent.notificationsPage.ready();
		await agent.notificationsPage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the offline page', async () => {
		// The offline settings page exists everywhere but iOS ({#if !isIos}).
		if (agent.platform === 'ios') {
			await expect(agent.settingsPage.offlineLink).not.toBeDisplayed();
			return;
		}
		await agent.settingsPage.offlineLink.click();
		await agent.offlinePage.ready();
		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the help page and displays the app version', async () => {
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();

		await agent.helpPage.versionItem.waitForExist();
		const versionText = await agent.helpPage.versionItem.getText();
		expect(versionText).toMatch(/\d+\.\d+\.\d+/);

		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
	});

	it('opens the contact-us page from help', async () => {
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();
		await agent.helpPage.contactUsLink.click();
		await agent.contactUsPage.ready();
		await agent.contactUsPage.back.click();
		await agent.helpPage.ready();
		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
	});
});
