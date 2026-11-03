import { TINY_PNG_BYTES } from '../helpers/images';
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

	it('attaches up to five screenshots to a contact-us message', async function () {
		// iOS opens the native photo picker, which no driver can answer.
		if (agent.platform === 'ios') this.skip();
		const screenshot = (name: string) => ({
			name,
			mimeType: 'image/png',
			bytes: TINY_PNG_BYTES,
		});
		await agent.settingsPage.helpLink.click();
		await agent.helpPage.ready();
		await agent.helpPage.contactUsLink.click();
		await agent.contactUsPage.ready();
		const page = agent.contactUsPage;

		const request = await page.pickScreenshots([
			screenshot('a.png'),
			screenshot('b.png'),
		]);
		expect(request.multiple).toBe(true);
		await expect(page.screenshots()).toBeElementsArrayOfSize(2);

		await page.pickScreenshots(
			['c.png', 'd.png', 'e.png', 'f.png'].map(screenshot),
		);
		await agent.toast.expectMessage(await agent.tr('errorTooManyScreenshots'));
		await expect(page.screenshots()).toBeElementsArrayOfSize(5);
		await expect(page.addScreenshot).not.toBeExisting();

		await page.removeScreenshot(0).click();
		await expect(page.screenshots()).toBeElementsArrayOfSize(4);
		await expect(page.addScreenshot).toBeExisting();

		await page.pickScreenshots([
			{ name: 'notes.txt', mimeType: 'text/plain', bytes: [104, 105] },
		]);
		await agent.toast.expectMessage(await agent.tr('errorUnsupportedImage'));
		await expect(page.screenshots()).toBeElementsArrayOfSize(4);

		await page.pickScreenshots([
			{ name: 'big.gif', mimeType: 'image/gif', size: 9 * 1024 * 1024 },
		]);
		await agent.toast.expectMessage(
			await agent.tr('errorScreenshotsTooLarge', { max: '8.0 MB' }),
		);
		await expect(page.screenshots()).toBeElementsArrayOfSize(4);

		await page.pickScreenshots([screenshot('g.png')]);
		await expect(page.screenshots()).toBeElementsArrayOfSize(5);

		await page.back.click();
		await agent.helpPage.ready();
		await agent.helpPage.back.click();
		await agent.settingsPage.ready();
	});
});
