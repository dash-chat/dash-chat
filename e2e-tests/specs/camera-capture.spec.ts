/**
 * Camera capture E2E — takes a picture from the edit-photo page and follows it
 * through to the saved profile avatar. The camera is observed through the
 * `window.__test` file-picker seam, which answers the input the app opens with
 * a known image in place of the one the OS camera would return.
 */
import { SOLID_PNG_BYTES, SOLID_PNG_RGB } from '../helpers/images';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO = {
	name: 'capture.png',
	mimeType: 'image/png',
	bytes: SOLID_PNG_BYTES,
};

describe('Camera capture', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'android' }]);
		await agent.createProfilePage.createProfile('Camera', 'Test');
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.profileLink.click();
		await agent.profilePage.ready();
	});

	it('offers a camera action alongside the gallery', async () => {
		await agent.profilePage.editPhoto.click();
		await agent.editPhotoPage.ready();

		await expect(agent.editPhotoPage.cameraButton).toBeDisplayed();
		await expect(agent.editPhotoPage.galleryButton).toBeDisplayed();

		await agent.editPhotoPage.close.click();
		await agent.profilePage.ready();
	});

	it('takes a picture and uses it as the profile photo', async () => {
		await agent.profilePage.editPhoto.click();
		await agent.editPhotoPage.ready();

		const request = await agent.editPhotoPage.takePhoto(PHOTO);
		expect(request.accept).toBe('image/*');
		expect(request.capture).toBe(true);

		await agent.editPhotoPage.save();
		await agent.profilePage.ready();

		expect(await agent.profilePage.avatarRgb()).toEqual(SOLID_PNG_RGB);
	});

	it('leaves the gallery action on the photo picker', async () => {
		await agent.profilePage.editPhoto.click();
		await agent.editPhotoPage.ready();

		const request = await agent.editPhotoPage.pickFromGallery();

		expect(request.capture).toBe(false);
		expect(request.accept).toContain('image/jpeg');

		await agent.editPhotoPage.close.click();
		await agent.profilePage.ready();
	});
});
