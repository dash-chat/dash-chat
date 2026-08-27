/**
 * Media attachments E2E — verifies that photos and files can be attached to a
 * message, sent, and rendered on both ends, and that the 16 MiB size cap is
 * enforced.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { tid } from '../helpers/selectors';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Media attachments', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Media');
		await agent2.createProfilePage.createProfile('Bob', 'Media');
		await exchangeContacts(agent1, agent2);
	});

	it('opens the desktop attach dropdown and renders the Photos and File items', async function () {
		if (agent1.platform !== 'desktop') this.skip();
		const composer = agent1.directChatPage.composer;
		await composer.openAttachMenu();
		expect(await composer.attachItemLabel('photos')).toBe(
			await agent1.tr('attachPhotos'),
		);
		expect(await composer.attachItemLabel('file')).toBe(
			await agent1.tr('attachFile'),
		);
		await composer.closeAttachMenu();
	});

	it('sends a single photo from Alice and renders on both ends', async () => {
		await agent1.directChatPage.composer.attachPhotos('single');
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('single');
		await agent2.directChatPage.messages.waitForPhotoMessage('single');
	});

	it('sends multiple photos with a caption', async () => {
		for (let i = 0; i < 3; i++) {
			await agent1.directChatPage.composer.attachPhotos('captioned');
		}
		await agent1.directChatPage.composer.type('three pics');
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForMessage('three pics');
		await agent2.directChatPage.messages.waitForMessage('three pics');
	});

	it('keeps the keyboard open when sending from the staged media page', async function () {
		if (agent1.platform === 'desktop') this.skip();
		const composer = agent1.directChatPage.composer;
		await composer.attachPhotos('kbd');
		await composer.focusStagedCaption();
		await composer.type('keyboard stays');
		await composer.sendFromStagedMediaPage();
		await composer.stagedMediaPage.waitForExist({ reverse: true });
		await agent1.directChatPage.messages.waitForMessage('keyboard stays');
		await agent1.waitUntil(() => composer.isInputFocused(), {
			timeoutMsg:
				'Composer input did not regain focus after sending staged media',
		});
		expect(await agent1.isKeyboardShown()).toBe(true);
	});

	it('sends a file attachment and renders on both ends', async () => {
		await agent1.directChatPage.composer.attachFile(
			'e2e-notes.txt',
			'hello from e2e',
			'text/plain',
		);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForFileMessage('e2e-notes.txt');
		await agent2.directChatPage.messages.waitForFileMessage('e2e-notes.txt');
	});

	it('rejects an attachment that exceeds the 16 MiB cap', async () => {
		const OVER_LIMIT = 16 * 1024 * 1024 + 1;
		await agent1.directChatPage.composer.attachFileOfSize(
			OVER_LIMIT,
			'too-big.bin',
		);
		await agent1.directChatPage.composer.send();

		await agent1.toast.expectMessageContaining('too large');

		// Draft survives the rejection so the user can remove the file.
		if (!(await agent1.directChatPage.composer.hasMediaPreview())) {
			throw new Error('Draft was cleared after rejection');
		}
		await agent1.directChatPage.composer.removeDraft();
	});

	it('appends photos picked separately instead of replacing', async () => {
		for (let i = 0; i < 3; i++) {
			await agent1.directChatPage.composer.attachPhotos('appended');
		}
		await agent1.directChatPage.composer.expectStagedPhotoCount(3);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('appended');
	});

	it('rejects a file while photos are staged', async () => {
		await agent1.directChatPage.composer.attachPhotos('rejected-with-file');
		await agent1.directChatPage.composer.attachFile(
			'mix.txt',
			'mix',
			'text/plain',
		);
		await agent1.toast.expectMessageContaining('along with files');
		await agent1.directChatPage.composer.expectStagedPhotoCount(1);
		await agent1.directChatPage.composer.removeDraft();
	});

	it('rejects adding photos while a file is staged', async () => {
		await agent1.directChatPage.composer.attachFile(
			'only.txt',
			'only',
			'text/plain',
		);
		await agent1.directChatPage.composer.attachPhotos('blocked-by-file');
		await agent1.toast.expectMessageContaining('one file at a time');
		await agent1.directChatPage.composer.expectStagedPhotoCount(0);
		await agent1.directChatPage.composer.removeDraft();
	});

	it('removes a single staged photo and clears all staged photos', async () => {
		const composer = agent1.directChatPage.composer;
		for (let i = 0; i < 3; i++) await composer.attachPhotos('staged');
		await composer.expectStagedPhotoCount(3);
		await composer.removeStagedPhoto(1);
		await composer.expectStagedPhotoCount(2);
		await composer.clearAll();
		await agent1.waitUntil(async () => !(await composer.hasMediaPreview()), {
			timeoutMsg: 'Preview still present after clear all',
		});
	});

	it('stages a pasted image', async () => {
		const composer = agent1.directChatPage.composer;
		await composer.pastePhotos('pasted');
		await composer.expectStagedPhotoCount(1);
		await composer.removeDraft();
	});

	it('stages dropped images, appending to the draft', async () => {
		const composer = agent1.directChatPage.composer;
		await composer.dropPhotos('dropped');
		await composer.dropPhotos('dropped');
		await composer.expectStagedPhotoCount(2);
		await composer.pastePhotos('pasted');
		await composer.expectStagedPhotoCount(3);
		await composer.clearAll();
		await agent1.waitUntil(async () => !(await composer.hasMediaPreview()), {
			timeoutMsg: 'Preview still present after clear all',
		});
	});

	it('caps staged photos at 32', async () => {
		const composer = agent1.directChatPage.composer;
		for (let i = 0; i < 32; i++) await composer.attachPhotos('capped');
		await composer.expectStagedPhotoCount(32);
		// The composer is at the cap, so the next photo is rejected.
		await composer.attachPhotos('capped');
		await agent1.toast.expectMessageContaining('cannot add any more');
		await composer.expectStagedPhotoCount(32);

		// Discard the bulky draft without sending 32 photos through the network.
		await composer.clearAll();
		await agent1.waitUntil(async () => !(await composer.hasMediaPreview()), {
			timeoutMsg: 'Preview still present after clear all',
		});
	});

	it('shows a retry control and recovers after a failed image load', async () => {
		await agent1.directChatPage.composer.attachPhotos('retry-blob');
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('retry-blob');
		await agent2.directChatPage.messages.waitForPhotoMessage('retry-blob');

		// Read the exact alt off the rendered img so forceBlobError matches precisely.
		const exactAlt = await agent2.execute(
			(photosSel: string, label: string) => {
				const imgs = Array.from(
					document.querySelectorAll(`${photosSel} img`),
				) as HTMLImageElement[];
				const img = imgs.find(el => el.alt.includes(label));
				return img?.alt ?? null;
			},
			tid('message-attachment-photos'),
			'retry-blob',
		);
		if (!exactAlt) throw new Error('Could not read alt from blob-image img');

		await agent2.execute((alt: string) => {
			window.__test.forceBlobError(alt);
		}, exactAlt);

		const retry = agent2.$(tid('blob-image-retry'));
		await retry.waitForDisplayed();

		await retry.click();
		await agent2.directChatPage.messages.waitForPhotoMessage('retry-blob');
	});

	it('does not offer Copy on a photo-only message', async () => {
		const message =
			await agent1.directChatPage.messages.messageWithPhoto('single');
		if (!message) throw new Error('Photo message "single" not found');
		await message.openActions();
		expect(await message.copyAction.isExisting()).toBe(false);
		expect(await message.deleteAction.isExisting()).toBe(true);
	});
});
