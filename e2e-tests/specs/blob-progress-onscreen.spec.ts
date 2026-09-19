/**
 * Only the attachments on screen are asked about their download. Two photos
 * held in their downloading state sit at either end of a chat long enough
 * that only one of them fits in the viewport at a time, and whichever one is
 * scrolled into view is the only blob the receiver polls.
 */
import { createProfilesAndExchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** Enough messages between the two photos to push the first one well out of
 * the viewport once the chat sits at its bottom. */
const FILLER_MESSAGES = 25;

describe('Blob progress polls only the attachments on screen', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfilesAndExchangeContacts({ Alice: agent1, Bob: agent2 });
	});

	after(async () => {
		await agent2.setBlobFetchPaused(false);
	});

	it('polls the visible photo and not the one scrolled out of view', async () => {
		const messages = agent2.directChatPage.messages;
		const composer = agent1.directChatPage.composer;
		await agent2.setBlobFetchPaused(true);

		await composer.attachNoisePhoto('top', 800, 600);
		await composer.send();
		await messages
			.photoProgressRing('top')
			.waitForDisplayed({ timeout: SYNC_TIMEOUT });
		for (let i = 0; i < FILLER_MESSAGES; i++) {
			await composer.sendMessage(`filler ${i}`);
		}
		await messages.waitForMessage(`filler ${FILLER_MESSAGES - 1}`);
		await composer.attachNoisePhoto('bottom', 800, 600);
		await composer.send();
		await messages
			.photoProgressRing('bottom')
			.waitForDisplayed({ timeout: SYNC_TIMEOUT });

		await messages.photoCell('bottom').scrollIntoView({ block: 'center' });
		expect(
			await messages.photoCell('top').isDisplayed({ withinViewport: true }),
		).toBe(false);
		await agent2.waitUntil(
			async () => (await agent2.blobPolledHashes()).length === 1,
			{ timeoutMsg: 'Expected exactly the bottom photo to be polled' },
		);
		const [bottomHash] = await agent2.blobPolledHashes();

		await messages.photoCell('top').scrollIntoView({ block: 'center' });
		expect(
			await messages.photoCell('bottom').isDisplayed({ withinViewport: true }),
		).toBe(false);
		await agent2.waitUntil(
			async () => {
				const polled = await agent2.blobPolledHashes();
				return polled.length === 1 && polled[0] !== bottomHash;
			},
			{ timeoutMsg: 'Expected exactly the top photo to be polled' },
		);
	});
});
