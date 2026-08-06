/**
 * Pure peer-to-peer sync, with no mailbox in the picture.
 *
 * The shared cloud mailbox is suspended before anything syncs, and the local
 * mailbox is off by default, so profile announcements, contact exchange, text
 * messages, and media all have to propagate over a direct p2p (iroh/mDNS)
 * connection between the two agents. This is the path exercised by real
 * device-to-device delivery and, unlike the rest of the suite, never falls back
 * to a mailbox relay.
 *
 * Skips against a remote environment mailbox, whose lifecycle we can't control.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Pure p2p sync (no mailbox)', () => {
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
		await agent1.createProfilePage.createProfile('Alice', 'P2P');
		await agent2.createProfilePage.createProfile('Bob', 'P2P');
		await exchangeContacts(agent1, agent2);
	});

	after(() => {
		if (isRemoteMailbox()) return;
		try {
			resumeMailbox();
		} catch {
			/* mailbox process already gone */
		}
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
});
