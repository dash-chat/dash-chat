/**
 * Pure peer-to-peer sync, with no mailbox in the picture.
 *
 * The shared cloud mailbox is suspended before anything syncs, and the local
 * mailbox is off by default, so profile announcements, contact exchange, text
 * messages, and media all have to propagate over a direct p2p (iroh/mDNS)
 * connection between the two agents. This is the path exercised by real
 * device-to-device delivery and, unlike the rest of the suite, never falls back
 * to a mailbox relay. The chat navbar chip says so: it reads "p2p" while the
 * other agent is online.
 *
 * Skips against a remote environment mailbox, whose lifecycle we can't control.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { MAILBOX_HUNG_MS, UI_TIMEOUT } from '../helpers/timeouts';
import { isRemoteMailbox, suspendMailbox } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Pure p2p sync (no mailbox)', function () {
	this.timeout(300_000);

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		// Take the mailbox down before any sync happens so contact exchange and
		// every message below must travel over a direct p2p connection.
		suspendMailbox();
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
	});

	it('both chips read p2p once the cloud is out of reach, and the dialog explains it', async () => {
		for (const agent of [agent1, agent2]) {
			await agent.directChatPage.connectionStatusIndicator.waitForStatus(
				'p2p',
				MAILBOX_HUNG_MS,
				'the chip did not read p2p with the cloud down and the peer connected',
			);
		}

		const chip = agent1.directChatPage.connectionStatusIndicator;
		await chip.expectDialog(
			await agent1.tr('connectionStatusP2pTitle'),
			await agent1.tr('connectionStatusP2pDescription'),
		);
	});

	it('syncs a text message agent1 → agent2 over p2p', async () => {
		await agent1.directChatPage.composer.sendMessage('hello over p2p');
		await agent2.directChatPage.messages.waitForMessage('hello over p2p');
	});

	it('syncs a reply agent2 → agent1 over p2p', async () => {
		await agent2.directChatPage.composer.sendMessage('reply over p2p');
		await agent1.directChatPage.messages.waitForMessage('reply over p2p');
	});

	it('syncs a photo message over p2p', async function () {
		const { composer } = agent1.directChatPage;
		await composer.attachPhotos('p2pphoto');
		await composer.type('photo over p2p');
		await composer.send();
		await agent1.directChatPage.messages.waitForPhotoMessage('p2pphoto');
		await agent2.directChatPage.messages.waitForMessage('photo over p2p');
		await agent2.directChatPage.messages.waitForPhotoMessage('p2pphoto');
	});

	it('the chip reads disconnected once the peer goes offline, and p2p again once it is back', async () => {
		const chip = agent1.directChatPage.connectionStatusIndicator;
		await agent2.stopApp();
		await chip.waitForStatus(
			'disconnected',
			UI_TIMEOUT,
			'the chip did not read disconnected after the peer went offline',
		);
		await agent2.startApp();
		await chip.waitForStatus(
			'p2p',
			UI_TIMEOUT,
			'the chip did not read p2p after the peer came back',
		);
	});
});
