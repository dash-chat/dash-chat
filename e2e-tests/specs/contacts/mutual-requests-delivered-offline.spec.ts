/**
 * Both peers add each other while nothing can carry the contact requests (no
 * mailbox, p2p off), then turn p2p on. The rebuilt nodes never resend the
 * stranded requests, so neither side accepts, yet the direct chat still syncs
 * over p2p: a message the peer received must still reach "delivered".
 *
 * Skips against a remote environment mailbox, whose lifecycle we can't control.
 */
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Mutual contact requests that never arrive', () => {
	let alice: Agent;
	let bob: Agent;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'desktop' },
		]);
		suspendMailbox();
		await Promise.all([alice.disableP2p(), bob.disableP2p()]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
		await Promise.all([alice.enableP2p(), bob.enableP2p()]);
	});

	after(() => {
		if (isRemoteMailbox()) return;
		resumeMailbox();
	});

	it('marks a message the peer received as delivered', async () => {
		await alice.directChatPage.composer.sendMessage('hello, stranded');
		await bob.directChatPage.messages.waitForMessage('hello, stranded');
		await alice.directChatPage.messages.waitForMessageStatus(
			'hello, stranded',
			['delivered'],
		);
	});
});
