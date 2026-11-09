/**
 * Message spotlight E2E — while a long-pressed message is spotlighted it is only
 * there to be looked at: touching it dismisses the overlay and nothing of its
 * own can be activated, like the snapshot Signal shows in its place.
 */
import type { Message } from '../helpers/components/messages';
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Message spotlight', () => {
	let agent1: Agent;
	let agent2: Agent;
	let message: Message;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'phone' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);

		const { composer, messages } = agent1.directChatPage;
		for (let i = 0; i < 3; i++) {
			await composer.attachPhotos('spotlight');
		}
		await composer.send();
		await messages.waitForPhotoMessage('spotlight');
		const photoMessage = await messages.messageWithPhoto('spotlight');
		if (!photoMessage) throw new Error('Photo message "spotlight" not found');
		message = photoMessage;
	});

	async function dismissByTappingTheMessage() {
		await message.tapByTouch();
		await message.actionsMenu.waitForDisplayed({ reverse: true });
	}

	it('closes when its photo is tapped, without opening the lightbox', async () => {
		await message.openActions();

		await dismissByTappingTheMessage();

		expect(await agent1.directChatPage.messages.lightbox.isOpen()).toBe(false);
	});

	it('does not start a reply when the message is swiped', async () => {
		await message.openActions();

		await message.swipeByTouch();

		await agent1.pause(1000);
		expect(await agent1.directChatPage.composer.replyBanner.isDisplayed()).toBe(
			false,
		);
		await dismissByTappingTheMessage();
	});

	it('does not start a reply when the long-press drags on without lifting', async () => {
		await message.holdThenDragByTouch();

		await message.actionsMenu.waitForDisplayed();
		await agent1.pause(1000);
		expect(await agent1.directChatPage.composer.replyBanner.isDisplayed()).toBe(
			false,
		);
		await dismissByTappingTheMessage();
	});

	it('keeps its photo out of keyboard reach', async () => {
		await message.openActions();

		expect(await message.photoTakesFocus()).toBe(false);

		await dismissByTappingTheMessage();
	});
});
