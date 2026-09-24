/**
 * A photo the iPhone already showed still shows after the app comes back from
 * the background (#607, reported as "the image stays black until I restart the
 * app", field log DASH-CHAT-3N). iOS tears the node down when the app leaves
 * the screen and rebuilds it on return; a chat reopened meanwhile asks for its
 * photos while the node is not back yet.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO = 'after-ios-resume';
/** Long enough for iOS to suspend the backgrounded app. */
const BACKGROUNDED_MS = 30_000;
/** The photo is already on the phone, so reading it back is local. */
const LOCAL_PHOTO_TIMEOUT = 30_000;

describe('Photo after the iPhone app comes back', function () {
	this.timeout(300_000);

	let alice: Agent;
	let bob: Agent;

	before(async function () {
		[alice, bob] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'ios' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
	});

	it('shows the photo when the chat is reopened right after coming back', async () => {
		await alice.directChatPage.composer.attachNoisePhoto(PHOTO, 1920, 1440);
		await alice.directChatPage.composer.type('a photo');
		await alice.directChatPage.composer.send();
		await bob.directChatPage.messages.waitForPhotoMessage(PHOTO);

		await bob.backgroundApp();
		await bob.pause(BACKGROUNDED_MS);
		await bob.startApp();
		await bob.directChatPage.back.click();
		await bob.homePage.ready();
		await bob.homePage.openChat('Alice');

		await bob.directChatPage.messages.waitForPhotoMessage(
			PHOTO,
			LOCAL_PHOTO_TIMEOUT,
		);
	});
});
