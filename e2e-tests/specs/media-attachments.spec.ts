/**
 * Media attachments E2E — verifies that photos and files can be attached to a
 * message, sent, and rendered on both ends, and that the 16 MiB size cap is
 * enforced.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Media attachments', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Media');
		await agent2.createProfilePage.createProfile('Bob', 'Media');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a single photo from Alice and renders on both ends', async () => {
		await agent1.directChatPage.attachPhotos(1);
		await agent1.directChatPage.sendComposer();
		await agent1.directChatPage.waitForPhotoMessage();
		await agent2.directChatPage.waitForPhotoMessage();
	});

	it('sends multiple photos with a caption', async () => {
		await agent1.directChatPage.attachPhotos(3);
		await agent1.execute(() => {
			const ta = document.querySelector(
				'[data-testid="message-input-textarea"]',
			) as HTMLTextAreaElement;
			const setter = Object.getOwnPropertyDescriptor(
				HTMLTextAreaElement.prototype,
				'value',
			)!.set!;
			setter.call(ta, 'three pics');
			ta.dispatchEvent(new Event('input', { bubbles: true }));
		});
		await agent1.directChatPage.sendComposer();
		await agent1.directChatPage.waitForMessage('three pics');
		await agent2.directChatPage.waitForMessage('three pics');
	});

	it('sends a file attachment and renders on both ends', async () => {
		await agent1.directChatPage.attachFile(
			'e2e-notes.txt',
			'hello from e2e',
			'text/plain',
		);
		await agent1.directChatPage.sendComposer();
		await agent1.directChatPage.waitForFileMessage('e2e-notes.txt');
		await agent2.directChatPage.waitForFileMessage('e2e-notes.txt');
	});

	it('rejects an attachment that exceeds the 16 MiB cap', async () => {
		const OVER_LIMIT = 16 * 1024 * 1024 + 1;
		await agent1.directChatPage.attachFileOfSize(OVER_LIMIT, 'too-big.bin');
		await agent1.directChatPage.sendComposer();

		await agent1.toast.expectMessageContaining('too large');

		// Draft survives the rejection so the user can remove the file.
		if (!(await agent1.directChatPage.hasMediaPreview())) {
			throw new Error('Draft was cleared after rejection');
		}
		await agent1.directChatPage.removeDraft();
	});

	it('appends photos picked separately instead of replacing', async () => {
		await agent1.directChatPage.attachPhotos(1);
		await agent1.directChatPage.attachPhotos(2);
		await agent1.directChatPage.expectStagedPhotoCount(3);
		await agent1.directChatPage.sendComposer();
		// waitForPhotoMessage matches photos from earlier tests instantly, so
		// wait for the staging strip to clear — that marks send completion and
		// keeps the next test from racing the in-flight draft.
		await agent1.waitUntil(
			async () => !(await agent1.directChatPage.hasMediaPreview()),
			{ timeoutMsg: 'Draft still staged after send completed' },
		);
		await agent1.directChatPage.waitForPhotoMessage();
	});

	it('rejects a file while photos are staged', async () => {
		await agent1.directChatPage.attachPhotos(1);
		await agent1.directChatPage.attachFile('mix.txt', 'mix', 'text/plain');
		await agent1.toast.expectMessageContaining('along with files');
		await agent1.directChatPage.expectStagedPhotoCount(1);
		await agent1.directChatPage.removeDraft();
	});

	it('rejects adding photos while a file is staged', async () => {
		await agent1.directChatPage.attachFile('only.txt', 'only', 'text/plain');
		await agent1.directChatPage.attachPhotos(1);
		await agent1.toast.expectMessageContaining('one file at a time');
		await agent1.directChatPage.expectStagedPhotoCount(0);
		await agent1.directChatPage.removeDraft();
	});

	it('removes a single staged photo and clears all staged photos', async () => {
		const composer = agent1.directChatPage.composer;
		await composer.attachPhotos(3);
		await composer.expectStagedPhotoCount(3);
		await composer.removeAttachmentButton(1).click();
		await composer.expectStagedPhotoCount(2);
		await composer.clearAttachments.click();
		await agent1.waitUntil(async () => !(await composer.hasMediaPreview()), {
			timeoutMsg: 'Preview still present after clear all',
		});
	});

	it('stages a pasted image', async () => {
		const composer = agent1.directChatPage.composer;
		await composer.pastePhotos(1);
		await composer.expectStagedPhotoCount(1);
		await composer.removeDraft();
	});

	it('stages dropped images, appending to the draft', async () => {
		const composer = agent1.directChatPage.composer;
		await composer.dropPhotos(2);
		await composer.expectStagedPhotoCount(2);
		await composer.pastePhotos(1);
		await composer.expectStagedPhotoCount(3);
		await composer.clearAttachments.click();
		await agent1.waitUntil(async () => !(await composer.hasMediaPreview()), {
			timeoutMsg: 'Preview still present after clear all',
		});
	});

	it('caps staged photos at 32 with partial accept', async () => {
		await agent1.directChatPage.attachPhotos(30);
		await agent1.directChatPage.expectStagedPhotoCount(30);
		await agent1.directChatPage.attachPhotos(5);
		await agent1.toast.expectMessageContaining('cannot add any more');
		await agent1.directChatPage.expectStagedPhotoCount(32);
		await agent1.directChatPage.attachPhotos(1);
		await agent1.toast.expectMessageContaining('cannot add any more');
		await agent1.directChatPage.expectStagedPhotoCount(32);

		// Leave and re-enter the chat to discard the bulky draft without
		// sending 32 photos through the network.
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.openChat('Bob');
		await agent1.directChatPage.ready();
	});
});
