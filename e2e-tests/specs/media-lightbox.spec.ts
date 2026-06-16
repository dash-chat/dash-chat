/**
 * Lightbox E2E — opening photos from a message bubble, navigating between
 * them (buttons, keyboard, filmstrip), and closing with focus restored.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Photo lightbox', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Lightbox');
		await agent2.createProfilePage.createProfile('Bob', 'Lightbox');
		await exchangeContacts(agent1, agent2);

		await agent1.directChatPage.composer.attachPhotos('lightbox', 3);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('lightbox');
	});

	afterEach(async () => {
		// Keep tests independent: a failed assertion mid-test can leave the
		// lightbox open (possibly zoomed — first Escape only exits zoom).
		for (const agent of [agent1, agent2]) {
			for (let i = 0; i < 2 && (await agent.lightbox.isOpen()); i++) {
				await agent.lightbox.pressKey('Escape');
			}
		}
	});

	it('opens the clicked photo and closes with the close button', async () => {
		await agent1.directChatPage.messages.photoCellButton().click();
		await agent1.lightbox.root.waitForExist();
		await agent1.lightbox.close.click();
		await agent1.waitUntil(async () => !(await agent1.lightbox.isOpen()), {
			timeoutMsg: 'Lightbox did not close via close button',
		});
	});

	it('navigates with arrows, keyboard, and filmstrip', async () => {
		await agent1.directChatPage.messages.photoCellButton().click();
		await agent1.lightbox.root.waitForExist();

		const first = await agent1.lightbox.imageSrc();
		await agent1.lightbox.next.click();
		await agent1.waitUntil(
			async () => (await agent1.lightbox.imageSrc()) !== first,
			{ timeoutMsg: 'Next button did not change the photo' },
		);

		const second = await agent1.lightbox.imageSrc();
		await agent1.lightbox.pressKey('ArrowRight');
		await agent1.waitUntil(
			async () => (await agent1.lightbox.imageSrc()) !== second,
			{ timeoutMsg: 'ArrowRight did not change the photo' },
		);

		// At the last photo the next button disappears.
		await agent1.waitUntil(
			async () => !(await agent1.lightbox.next.isExisting()),
			{ timeoutMsg: 'Next button still visible on last photo' },
		);

		await agent1.lightbox.pressKey('ArrowLeft');
		await agent1.waitUntil(
			async () => (await agent1.lightbox.imageSrc()) === second,
			{ timeoutMsg: 'ArrowLeft did not go back' },
		);

		await agent1.lightbox.thumb(0).click();
		await agent1.waitUntil(
			async () => (await agent1.lightbox.imageSrc()) === first,
			{ timeoutMsg: 'Filmstrip thumb did not select the first photo' },
		);

		await agent1.lightbox.pressKey('Escape');
		await agent1.waitUntil(async () => !(await agent1.lightbox.isOpen()), {
			timeoutMsg: 'Escape did not close the lightbox',
		});
	});

	it('restores focus to the triggering photo on close', async () => {
		await agent1.directChatPage.messages.photoCellButton().click();
		await agent1.lightbox.root.waitForExist();
		await agent1.lightbox.pressKey('Escape');
		await agent1.waitUntil(async () => !(await agent1.lightbox.isOpen()), {
			timeoutMsg: 'Lightbox did not close',
		});
		const focusRestored = await agent1.execute(() => {
			const active = document.activeElement;
			return !!active?.closest('[data-testid="message-attachment-photos"]');
		});
		if (!focusRestored) {
			throw new Error('Focus was not restored to the photo cell');
		}
	});

	it('opens on the receiving side too', async () => {
		await agent2.directChatPage.messages.waitForPhotoMessage('lightbox');
		await agent2.directChatPage.messages.photoCellButton().click();
		await agent2.lightbox.root.waitForExist();
		await agent2.lightbox.close.click();
	});
});
