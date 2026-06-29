/**
 * Voice notes E2E — verifies a voice message can be staged (a synthetic WAV is
 * injected, since the headless WebKitGTK harness has no microphone), sent, and
 * rendered as a playable waveform bubble on both ends.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Voice notes', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Voice');
		await agent2.createProfilePage.createProfile('Bob', 'Voice');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a voice note from Alice and renders on both ends', async () => {
		// Metadata duration (4s) deliberately overshoots the real audio (2s) so the
		// later test can prove the scrubber tracks the audio's own duration.
		await agent1.directChatPage.composer.recordVoiceNote(4000, 2000);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForVoiceMessage();
		await agent2.directChatPage.messages.waitForVoiceMessage();
	});

	it('renders the played region visibly distinct from the unplayed region', async () => {
		// wavesurfer composites progressColor onto the wave canvas with `source-in`,
		// so a translucent waveColor collapses the two to near-identical alpha and
		// the progress is invisible. The played bars must contrast clearly.
		const { unplayed, played } =
			await agent2.directChatPage.messages.voiceBarLuminance();
		expect(Math.abs(played - unplayed)).toBeGreaterThan(30);
	});

	it('plays the received voice note and advances the waveform progress', async () => {
		const messages = agent2.directChatPage.messages;
		await messages.voicePlayButton.waitForClickable();
		await messages.voicePlayButton.click();
		await agent2.waitUntil(async () => (await messages.voiceProgress()) > 0.1, {
			timeoutMsg: 'Waveform progress did not advance during playback',
		});
		const mid = await messages.voiceProgress();
		await agent2.waitUntil(
			async () => (await messages.voiceProgress()) > mid + 0.1,
			{ timeoutMsg: 'Waveform progress stalled during playback' },
		);
	});

	it('maps progress to the real audio duration, not the recorded metadata', async () => {
		const messages = agent2.directChatPage.messages;
		// Audio is really 2s but metadata claims 4s; seeking to the real midpoint
		// must fill ~50% of the scrubber (driven by audio.duration), not ~25%.
		const realFraction = await messages.voiceSeekFraction(0.5);
		expect(realFraction).toBeGreaterThan(0);
		await agent2.waitUntil(
			async () => Math.abs((await messages.voiceProgress()) - 0.5) < 0.1,
			{ timeoutMsg: 'Scrubber did not track the real audio duration' },
		);
	});
});
