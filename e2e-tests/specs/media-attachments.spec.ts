/**
 * Media attachments E2E — verifies that photos and files can be attached to a
 * message, sent, and rendered on both ends. Mirrors the structure of
 * `full-flow.spec.ts`.
 */

import {
	type Agent,
	exchangeContacts,
	setupAgent,
} from '../helpers/setup-agents';

describe('Media attachments', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfile('Alice', 'Media');
		await agent2.createProfile('Bob', 'Media');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a single photo from Alice and renders on both ends', async () => {
		await agent1.openDirectChat('Bob Media');
		await agent1.attachPhotos(1);
		await agent1.sendComposer();
		await agent1.waitForPhotoMessage();

		await agent2.openDirectChat('Alice Media');
		await agent2.waitForPhotoMessage();
	});

	it('sends multiple photos with a caption', async () => {
		await agent1.sendMessage(''); // ensure composer is empty (no-op send)
		await agent1.attachPhotos(3);
		// Caption goes via the same textarea path as plain text.
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
		await agent1.sendComposer();
		await agent1.waitForMessage('three pics');
		await agent2.waitForMessage('three pics');
	});

	it('sends a file attachment and renders on both ends', async () => {
		await agent1.attachFile('e2e-notes.txt', 'hello from e2e', 'text/plain');
		await agent1.sendComposer();
		await agent1.waitForFileMessage('e2e-notes.txt');
		await agent2.waitForFileMessage('e2e-notes.txt');
	});
});
