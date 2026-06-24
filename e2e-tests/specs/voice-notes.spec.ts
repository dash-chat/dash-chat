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
		await agent1.directChatPage.composer.recordVoiceNote(3000);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForVoiceMessage();
		await agent2.directChatPage.messages.waitForVoiceMessage();
	});

	it('exposes a clickable play control on the received voice note', async () => {
		const playButton = agent2.directChatPage.messages.voicePlayButton;
		await playButton.waitForClickable();
		await playButton.click();
	});
});
