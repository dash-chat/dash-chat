/**
 * A photo whose sender goes away mid-upload (#600, field report DASH-CHAT-4F).
 * On a weak uplink the message lands on the mailbox within a second, but its
 * photo takes about a minute to follow; a sender whose app is closed or frozen
 * in between leaves the receiver a message whose photo only the sender holds,
 * so it can only arrive once the sender is back. P2P is off on both agents so
 * mailboxes are the only way the photo can travel. A local hub the sender
 * never saw comes up while she is away, as on the field test's LAN: the
 * receiver forwards her message to it.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type LocalHub, spawnLocalHub, stopLocalHub } from '../setup/local-hub';
import {
	healMailboxLink,
	isRemoteMailbox,
	throttleMailboxUploads,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO = 'sent-then-gone';
const CAPTION = 'sent right before closing the app';

describe('Photo whose sender goes away mid-upload', function () {
	this.timeout(600_000);

	let alice: Agent;
	let bob: Agent;
	let linkDegraded = false;
	let hub: LocalHub | undefined;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
		await alice.disableP2p();
		await bob.disableP2p();
		await bob.stopApp();
	});

	after(async () => {
		if (linkDegraded) await healMailboxLink();
		if (hub !== undefined) await stopLocalHub(hub);
		await alice?.startApp();
		await bob?.startApp();
	});

	it('shows Bob the photo once Alice is back', async () => {
		await throttleMailboxUploads();
		linkDegraded = true;
		await alice.directChatPage.composer.attachNoisePhoto(PHOTO, 1920, 1440);
		await alice.directChatPage.composer.type(CAPTION);
		await alice.directChatPage.composer.send();
		await alice.directChatPage.messages.waitForMessageStatus(CAPTION, [
			'mailbox',
		]);
		await alice.stopApp();
		await healMailboxLink();
		linkDegraded = false;
		hub = await spawnLocalHub('media-sender-offline');

		await bob.startApp();
		await bob.homePage.ready();
		await bob.homePage.openChat('Alice');
		await bob.directChatPage.messages.waitForMessage(CAPTION);

		await alice.startApp();
		await bob.directChatPage.messages.waitForPhotoMessage(PHOTO);
	});
});
