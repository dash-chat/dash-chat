/**
 * A voice note whose blob has not arrived yet: the receiver's play button is
 * wrapped in a progress ring, a tap while it is still downloading is answered
 * with a toast, and once the blob lands the ring goes away and the note plays.
 */
import { createProfilesAndExchangeContacts } from '../../helpers/flows/exchange-contacts';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Voice message download progress', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfilesAndExchangeContacts({ Alice: agent1, Bob: agent2 });
	});

	it('shows a progress ring on the receiver until the voice blob arrives', async () => {
		const messages = agent2.directChatPage.messages;
		await agent2.setBlobFetchPaused(true);
		try {
			await agent1.directChatPage.composer.recordVoiceMessage(3000);
			await agent1.directChatPage.composer.send();
			await agent1.directChatPage.messages.waitForVoiceMessage();

			await messages
				.voiceProgressRing()
				.waitForDisplayed({ timeout: SYNC_TIMEOUT });
			await messages.voicePlayButton.click();
			await agent2.toast.expectMessageContaining(
				await agent2.tr('fileStillDownloading'),
			);
			await messages.voiceProgressRing().waitForDisplayed();
		} finally {
			await agent2.setBlobFetchPaused(false);
		}
		await messages
			.voiceProgressRing()
			.waitForDisplayed({ reverse: true, timeout: MEDIA_SYNC_TIMEOUT });
		await messages.voicePlayButton.click();
		await agent2.waitUntil(async () => (await messages.voiceProgress()) > 0.1, {
			timeoutMsg: 'Waveform progress did not advance during playback',
		});
	});
});
