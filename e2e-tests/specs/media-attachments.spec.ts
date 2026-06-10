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
});
