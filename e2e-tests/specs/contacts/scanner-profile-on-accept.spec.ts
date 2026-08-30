import { navigateToAddContact } from '../../helpers/flows/exchange-contacts';
import { tid } from '../../helpers/selectors';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';

async function waitForTextContent(
	agent: Agent,
	selector: string,
	text: string,
): Promise<void> {
	await agent.waitUntil(async () =>
		agent.execute(
			(sel: string, t: string) => window.__test.hasText(sel, t),
			selector,
			text,
		),
	);
}

describe('Scanner profile on accept', () => {
	let alice: Agent; // the scanner
	let bob: Agent; // the scanned contact who accepts
	let bobCode: string;
	let mailboxSuspended = false;

	before(async function () {
		// Suspending the shared mailbox is impossible against a remote environment
		// mailbox, so the offline window this relies on can't be created there.
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await Promise.all([
			alice.createProfilePage.createProfile('Alice', 'Test'),
			bob.createProfilePage.createProfile('Bob', 'Test'),
		]);
		// Read Bob's contact code while he's online (generating it sets up his
		// inbox gossip topic, which needs a live endpoint), then take him fully
		// offline: suspend the mailbox so nothing relays his profile, and close
		// his app so he can't sync it over p2p either.
		await navigateToAddContact(bob);
		bobCode = await bob.addContactPage.getAddContactLink();
		suspendMailbox();
		mailboxSuspended = true;
		await bob.stopApp();
	});

	after(() => {
		if (mailboxSuspended) {
			try {
				resumeMailbox();
			} catch {
				/* ignore */
			}
			mailboxSuspended = false;
		}
	});

	it('shows the scanned QR name with a placeholder avatar while the contact is offline', async () => {
		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobCode);
		await alice.directChatPage.ready();

		// The truncated name from the QR shows immediately; the full profile can't
		// arrive yet, so the avatar stays the person placeholder.
		await waitForTextContent(alice, tid('direct-chat-peer-header'), 'Bob Test');
		expect(await alice.directChatPage.peerAvatarIsPlaceholder()).toBe(true);
	});

	it("resolves the scanner's peer profile from the cached accept payload", async () => {
		// Bring Bob back online and accept Alice's request. Bob is the scanned
		// contact; Alice is the scanner, so she never receives a ContactRequest
		// from Bob. Her only source for Bob's profile at accept time is the
		// ContactRequestAccept payload Bob sends back.
		await bob.startApp();
		await bob.homePage.ready();
		const aliceRow = bob.homePage.chatListItem('Alice Test');
		await aliceRow.waitForExist({ timeout: SYNC_TIMEOUT });
		await aliceRow.click();
		await bob.directChatPage.acceptButton.waitForExist();
		await bob.directChatPage.acceptContactRequest();

		// Bob sees Alice's full profile (carried in the contact request).
		await waitForTextContent(bob, tid('direct-chat-peer-header'), 'Alice Test');
		await bob.directChatPage.waitForPeerProfile();

		// Wait until Alice's side has processed the accept and rendered Bob's
		// real profile. The accept carries Bob's profile, which is now cached
		// locally; Bob can go offline immediately after without blocking the
		// avatar resolution.
		await alice.directChatPage.waitForPeerProfile();

		// Stop Bob now that the cached profile has already resolved. This
		// prevents Alice from syncing Bob's announcements topic, which was the
		// only fallback source for his profile before the cached-profile fix.
		await bob.stopApp();

		// Verify the avatar stayed resolved after Bob went offline — i.e. it did
		// not depend on live announcements sync.
		expect(await alice.directChatPage.peerAvatarIsPlaceholder()).toBe(false);
	});
});
