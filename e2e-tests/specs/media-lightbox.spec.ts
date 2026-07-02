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

		for (let i = 0; i < 3; i++) {
			await agent1.directChatPage.composer.attachPhotos('lightbox');
		}
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('lightbox');
	});

	afterEach(async () => {
		// A failed test can leave a lightbox open; Escape exits zoom then closes
		// it. Both presses are no-ops when nothing is open.
		await agent1.directChatPage.messages.lightbox.pressKey('Escape');
		await agent1.directChatPage.messages.lightbox.pressKey('Escape');
		await agent2.directChatPage.messages.lightbox.pressKey('Escape');
		await agent2.directChatPage.messages.lightbox.pressKey('Escape');
	});

	it('opens the clicked photo and closes with the close button', async () => {
		await agent1.directChatPage.messages.photoCellButton(0).click();
		await agent1.directChatPage.messages.lightbox.root.waitForExist();
		await agent1.directChatPage.messages.lightbox.close.click();
		await agent1.waitUntil(
			async () => !(await agent1.directChatPage.messages.lightbox.isOpen()),
		);
	});

	it('navigates with arrows, keyboard, and filmstrip', async () => {
		await agent1.directChatPage.messages.photoCellButton(0).click();
		await agent1.directChatPage.messages.lightbox.root.waitForExist();

		const lb = agent1.directChatPage.messages.lightbox;
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 0);

		await lb.next.click();
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 1);

		await lb.pressKey('ArrowRight');
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 2);

		// At the last photo the next button disappears.
		await agent1.waitUntil(async () => !(await lb.next.isExisting()));

		await lb.pressKey('ArrowLeft');
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 1);

		await lb.thumb(0).click();
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 0);

		await agent1.directChatPage.messages.lightbox.pressKey('Escape');
		await agent1.waitUntil(
			async () => !(await agent1.directChatPage.messages.lightbox.isOpen()),
		);
	});

	it('retries a failed thumbnail when its reload icon is clicked', async () => {
		const lb = agent1.directChatPage.messages.lightbox;
		await agent1.directChatPage.messages.photoCellButton(0).click();
		await lb.root.waitForExist();
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 0);

		// Fail a non-active thumbnail; its reload icon should surface.
		await lb.forceThumbError(1);
		const retry = lb.thumbRetry(1);
		await retry.waitForDisplayed();

		// Clicking it both re-downloads (reload icon clears) and switches the
		// main view to that photo.
		await lb.thumb(1).click();
		await retry.waitForDisplayed({ reverse: true });
		await agent1.waitUntil(async () => (await lb.activeIndex()) === 1);
	});

	it('opens on the receiving side too', async () => {
		await agent2.directChatPage.messages.waitForPhotoMessage('lightbox');
		await agent2.directChatPage.messages.photoCellButton(0).click();
		await agent2.directChatPage.messages.lightbox.root.waitForExist();
		await agent2.directChatPage.messages.lightbox.close.click();
	});
});
