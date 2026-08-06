import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { tid } from '../helpers/selectors';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

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

describe('Contact profile disclosure', () => {
	let alice: Agent; // the scanned contact
	let bob: Agent; // the scanner
	let aliceCode: string;
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
		// Read Alice's contact code while she's online (generating it sets up her
		// inbox gossip topic, which needs a live endpoint), then take her fully
		// offline: suspend the mailbox so nothing relays her profile, and close her
		// app so she can't sync it over p2p either.
		await navigateToAddContact(alice);
		aliceCode = await alice.addContactPage.getAddContactLink();
		suspendMailbox();
		mailboxSuspended = true;
		await alice.stopApp();
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
		await navigateToAddContact(bob);
		await bob.addContactPage.enterAddContactLink(aliceCode);
		await bob.directChatPage.ready();

		// The truncated name from the QR shows immediately; the full profile can't
		// arrive yet, so the avatar stays the person placeholder.
		await waitForTextContent(bob, tid('direct-chat-peer-header'), 'Alice Test');
		expect(await bob.directChatPage.peerAvatarIsPlaceholder()).toBe(true);
	});

	it('shows the full profile with a real avatar once the contact comes online and accepts', async () => {
		// Bring Alice back online and accept Bob's request; p2p (never disabled)
		// then syncs both full profiles.
		await alice.startApp();
		await alice.homePage.ready();
		const bobRow = alice.homePage.chatListItem('Bob Test');
		await bobRow.waitForExist({ timeout: SYNC_TIMEOUT });
		await bobRow.click();
		await alice.directChatPage.acceptButton.waitForExist();
		await alice.directChatPage.acceptContactRequest();

		// Alice sees Bob's full profile (carried in the contact request).
		await waitForTextContent(alice, tid('direct-chat-peer-header'), 'Bob Test');
		await alice.waitUntil(async () =>
			!(await alice.directChatPage.peerAvatarIsPlaceholder()),
		);

		// Bob's view of Alice resolves from the QR placeholder to her real profile
		// once the accept, contact establishment and Alice's announcements sync over
		// p2p.
		await bob.waitUntil(async () =>
			!(await bob.directChatPage.peerAvatarIsPlaceholder()),
		);
	});
});
