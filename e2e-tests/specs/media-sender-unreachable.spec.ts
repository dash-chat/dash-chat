/**
 * A photo whose sender stays online but cannot be reached (#600, field report
 * DASH-CHAT-4F). A phone off Wi-Fi reaches its mailbox but cannot be dialed
 * back, by the mailbox or by other phones; here that phone keeps its mailbox
 * over USB (adb reverse) with Wi-Fi off. Its photo upload, crawling over a weak
 * uplink, drops when the connection does, and the photo can then only reach
 * the mailbox if the sender uploads it again.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	cutMailboxLink,
	healMailboxLink,
	isRemoteMailbox,
	throttleMailboxUploads,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO = 'unreachable-sender';
const CAPTION = 'sent from a phone off wifi';
/** Long enough for the dropped connection to fail the upload in flight. */
const CONNECTION_DROP_MS = 3_000;

describe('Photo whose sender stays online but cannot be reached', function () {
	this.timeout(600_000);

	let alice: Agent;
	let bob: Agent;
	let linkDegraded = false;
	let wifiOff = false;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'desktop' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
		await alice.disableWifi();
		wifiOff = true;
	});

	after(async () => {
		if (linkDegraded) await healMailboxLink();
		if (wifiOff) await alice.enableWifi();
	});

	it('shows Bob the photo while Alice stays online', async () => {
		await throttleMailboxUploads();
		linkDegraded = true;
		await alice.directChatPage.composer.attachNoisePhoto(PHOTO, 1920, 1440);
		await alice.directChatPage.composer.type(CAPTION);
		await alice.directChatPage.composer.send();
		await alice.directChatPage.messages.waitForMessageStatus(CAPTION, [
			'mailbox',
		]);
		await cutMailboxLink();
		await alice.pause(CONNECTION_DROP_MS);
		await healMailboxLink();
		linkDegraded = false;

		await bob.directChatPage.messages.waitForMessage(CAPTION);
		await bob.directChatPage.messages.waitForPhotoMessage(PHOTO);
	});
});
