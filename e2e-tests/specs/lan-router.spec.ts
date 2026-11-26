/**
 * The Dash Router switch in the developer settings, and what it turns on.
 *
 * Alice sends a contact request to someone who is not running, and stops
 * before they come back. The cloud mailbox is suspended, so the request can
 * only reach them through the third agent, who is running throughout and is
 * no part of it. With the routers off that agent carries nothing; with them
 * on it does.
 *
 * Skips against a remote environment mailbox, whose lifecycle we can't control.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import {
	openDeveloperSettings,
	returnToChatList,
	unlockDeveloperSettings,
} from '../helpers/flows/developer-settings';
import {
	contactLinkOf,
	navigateToAddContact,
} from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** Several times what a relay takes to copy a request written with the
 *  router on (under a second). */
const A_RELAY_HAS_COPIED_MS = 5_000;

async function turnRouterOn(agent: Agent): Promise<void> {
	await openDeveloperSettings(agent);
	await agent.developerPage.setLanRouterEnabled(true);
	await returnToChatList(agent);
}

describe('Dash Router', function () {
	this.timeout(300_000);

	let alice: Agent;
	let bob: Agent;
	let carol: Agent;
	let mailboxSuspended = false;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[alice, bob, carol] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: alice, Bob: bob, Carol: carol });
		suspendMailbox();
		mailboxSuspended = true;
	});

	after(() => {
		if (mailboxSuspended) resumeMailbox();
	});

	it('the router is off until the user turns it on', async () => {
		for (const agent of [alice, bob, carol]) {
			await unlockDeveloperSettings(agent);
			expect(await agent.developerPage.lanRouterEnabled()).toBe(false);
			await returnToChatList(agent);
		}
	});

	it('with the routers off, a request sent while its recipient is away never arrives', async () => {
		const carolLink = await contactLinkOf(carol);
		await carol.stopApp();

		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(carolLink);
		await alice.directChatPage.ready();
		await alice.pause(A_RELAY_HAS_COPIED_MS);
		await alice.stopApp();

		await carol.startApp();
		await carol.pause(A_RELAY_HAS_COPIED_MS);
		expect(await carol.homePage.hasChatListItem('Alice')).toBe(false);
	});

	it('with the routers on, a device in between carries it', async () => {
		await alice.startApp();
		await Promise.all([alice, bob, carol].map(turnRouterOn));
		const bobLink = await contactLinkOf(bob);
		await bob.stopApp();

		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobLink);
		await alice.directChatPage.ready();
		await alice.pause(A_RELAY_HAS_COPIED_MS);
		await alice.stopApp();

		await bob.startApp();
		await bob.homePage
			.chatListItem('Alice')
			.waitForExist({ timeout: SYNC_TIMEOUT });
	});

	it('the switch is still on after a restart', async () => {
		await openDeveloperSettings(bob);
		expect(await bob.developerPage.lanRouterEnabled()).toBe(true);
	});
});
