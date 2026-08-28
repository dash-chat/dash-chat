// A synthetic WAV is injected throughout: the headless WebKitGTK harness has no
// microphone.
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Voice messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Voice');
		await agent2.createProfilePage.createProfile('Bob', 'Voice');
		await exchangeContacts(agent1, agent2);
	});

	it('sends a voice message from Alice and renders on both ends', async () => {
		// Metadata duration (4s) deliberately overshoots the real audio (2s) so the
		// later test can prove the scrubber tracks the audio's own duration.
		await agent1.directChatPage.composer.recordVoiceMessage(4000, 2000);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForVoiceMessage();
		await agent2.directChatPage.messages.waitForVoiceMessage();
	});

	it('plays the received voice message and advances the waveform progress', async () => {
		const messages = agent2.directChatPage.messages;
		await messages.voicePlayButton.waitForClickable();
		await messages.voicePlayButton.click();
		await agent2.waitUntil(async () => (await messages.voiceProgress()) > 0.1, {
			timeoutMsg: 'Waveform progress did not advance during playback',
		});
	});

	it('shows a spinner while loading and toasts when the audio fails to load', async () => {
		// Delayed so the spinner is observable before the error toast.
		const messages = agent1.directChatPage.messages;
		await messages.waitForVoiceMessage();
		await messages.failNextVoiceLoad(1500);
		await messages.voicePlayButton.click();
		await agent1.waitUntil(() => messages.voicePlayLoading(), {
			timeoutMsg: 'Voice play button did not show its loading spinner',
		});
		await agent1.toast.expectMessageContaining(
			await agent1.tr('voicePlayFailed'),
		);
		await agent1.waitUntil(async () => !(await messages.voicePlayLoading()), {
			timeoutMsg: 'Voice play button stayed in its loading state after failure',
		});
	});

	it('maps progress to the real audio duration, not the recorded metadata', async () => {
		const messages = agent2.directChatPage.messages;
		// Audio is really 2s but metadata claims 4s; seeking to the real midpoint
		// must fill ~50% of the scrubber (driven by audio.duration), not ~25%.
		const realFraction = await messages.voiceSeekFraction(0.5);
		expect(realFraction).toBeCloseTo(0.5, 1);
		await agent2.waitUntil(
			async () => Math.abs((await messages.voiceProgress()) - 0.5) < 0.1,
			{ timeoutMsg: 'Scrubber did not track the real audio duration' },
		);
	});
});
