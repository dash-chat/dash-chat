import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { tid } from '../helpers/selectors';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgent } from '../setup/setup-agents';

async function waitForTextContent(
	agent: Agent,
	selector: string,
	text: string,
): Promise<void> {
	await agent.waitUntil(
		async () =>
			agent.execute(
				(sel: string, t: string) => window.__test.hasText(sel, t),
				selector,
				text,
			),
		{ timeout: 15_000 },
	);
}

describe('Waiting-for-profile placeholder', () => {
	let agent1: Agent;
	let agent2: Agent;
	let waitingText: string;
	let contactCode1: string;
	let mailboxSuspended = false;

	before(async function () {
		// The placeholder window only exists while the mailbox is suspended,
		// which is impossible against a remote environment mailbox.
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await Promise.all([
			agent1.createProfilePage.createProfile('Alice', 'Test'),
			agent2.createProfilePage.createProfile('Bob', 'Test'),
		]);
		waitingText = await agent2.tr('waitingForProfile');
		// Fetch agent1's contact code while its p2p is still up (generating the
		// code sets up agent1's inbox gossip topic, which needs a live endpoint).
		await navigateToAddContact(agent1);
		contactCode1 = await agent1.addContactPage.getAddContactLink();
		// Hold agent2 in the pre-sync state so the "waiting for profile"
		// placeholder stays visible: suspend the shared mailbox AND cut agent1
		// off from p2p. Without disabling p2p, the two agents sync agent1's
		// profile directly over the iroh relay (mDNS is already off in e2e
		// builds), bypassing the suspended mailbox and racing the assertions.
		await agent1.disableP2p();
		suspendMailbox();
		mailboxSuspended = true;
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

	it('shows the placeholder on direct-chat after one-sided contact addition', async () => {
		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterAddContactLink(contactCode1);
		await agent2.directChatPage.ready();

		await waitForTextContent(
			agent2,
			tid('direct-chat-peer-header'),
			waitingText,
		);
		await waitForTextContent(
			agent2,
			tid('direct-chat-settings-link'),
			waitingText,
		);
	});

	it('shows the placeholder on chat-settings', async () => {
		await agent2.directChatPage.settingsLink.click();
		await waitForTextContent(
			agent2,
			tid('chat-settings-peer-header'),
			waitingText,
		);
	});

	it('shows the placeholder on the home chat-list row', async () => {
		await agent2.chatSettingsPage.back.click();
		await agent2.directChatPage.ready();
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();
		await waitForTextContent(agent2, tid('all-chats-row'), waitingText);
	});
});
