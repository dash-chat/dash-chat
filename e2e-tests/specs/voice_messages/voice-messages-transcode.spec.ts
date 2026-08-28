// Unlike voice-messages.spec.ts (which injects a WAV draft directly), this runs
// a synthesized WAV through the real `transcode_voice_message` command, so it
// exercises the Opus transcode on send and the Rust Opus→WAV decode on playback.
// The headless WebKitGTK harness has no microphone, so only the native capture
// itself is bypassed.
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Voice messages (Opus transcode)', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Opus');
		await agent2.createProfilePage.createProfile('Bob', 'Opus');
		await exchangeContacts(agent1, agent2);
	});

	it('transcodes a recording to Ogg/Opus that is smaller than the source', async () => {
		const result =
			await agent1.directChatPage.composer.recordRealVoiceMessage(1000);
		expect(result.isOgg).toBe(true);
		expect(result.opusBytes).toBeLessThan(result.wavBytes);
		// Duration comes from the decoded audio, so it tracks the ~1s source.
		expect(Math.abs(result.durationMs - 1000)).toBeLessThan(150);
	});

	it('sends the Opus message and renders it on both ends', async () => {
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForVoiceMessage();
		await agent2.directChatPage.messages.waitForVoiceMessage();
	});

	it('plays the received Opus message via the Rust WAV decode', async () => {
		const messages = agent2.directChatPage.messages;
		await messages.voicePlayButton.waitForClickable();
		await messages.voicePlayButton.click();
		await agent2.waitUntil(async () => (await messages.voiceProgress()) > 0.1, {
			timeoutMsg: 'Waveform progress did not advance during Opus playback',
		});
	});
});
