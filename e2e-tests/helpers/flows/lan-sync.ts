import { launchEphemeralAgent } from '../../setup/ephemeral-agent';
import { cutMailboxLink, healMailboxLink } from '../../setup/mailbox-control';
import type { Agent } from '../../setup/setup-agents';
import { exchangeContacts } from './exchange-contacts';

export interface NamedAgent {
	agent: Agent;
	/** The profile name the other agents list it under. */
	name: string;
}

/**
 * Launch a short-lived desktop agent, make it a contact of each of `hosts`
 * over the mailbox and greet the last of them, then kill it. What it leaves
 * behind is what every mailbox author leaves in production: one more entry in
 * each host's address book that will never answer again.
 */
export async function meetVisitor(
	round: number,
	hosts: NamedAgent[],
): Promise<void> {
	const visitor = await launchEphemeralAgent();
	try {
		await visitor.agent.createProfilePage.createProfile(`Visitor ${round}`);
		for (const host of hosts) {
			await exchangeContacts([host.agent, visitor.agent]);
		}
		const greeting = `Hello from visitor ${round}`;
		await visitor.agent.directChatPage.composer.sendMessage(greeting);
		await hosts[hosts.length - 1].agent.directChatPage.messages.waitForMessage(
			greeting,
		);
	} finally {
		await visitor.kill();
	}
}

/** Run `during` with the mailbox unreachable, so only a direct connection
 *  between the agents can carry what it sends. */
export async function withMailboxCut(
	during: () => Promise<void>,
): Promise<void> {
	await cutMailboxLink();
	try {
		await during();
	} finally {
		await healMailboxLink();
	}
}

/** Open the direct chat between `a` and `b` on both, and have each deliver a
 *  message to the other. Assumes the mailbox is out of the picture. */
export async function expectSyncBothWays(
	a: NamedAgent,
	b: NamedAgent,
	label: string,
): Promise<void> {
	await openChatWith(a.agent, b.name);
	await openChatWith(b.agent, a.name);

	const pingMs = await deliver(a, b, `Ping ${label}`);
	const pongMs = await deliver(b, a, `Pong ${label}`);
	console.log(
		`[lan-sync] ${label}: ${a.name}→${b.name} ${pingMs}ms, ${b.name}→${a.name} ${pongMs}ms`,
	);
}

/** Send `text` in the open chat and wait for it on the other side; how long
 *  that took, from sending to seeing it arrive. */
async function deliver(
	from: NamedAgent,
	to: NamedAgent,
	text: string,
): Promise<number> {
	await from.agent.directChatPage.composer.sendMessage(text);
	const sentAt = Date.now();
	await to.agent.directChatPage.messages.waitForMessage(text);
	return Date.now() - sentAt;
}

export async function openChatWith(
	agent: Agent,
	contactName: string,
): Promise<void> {
	if (await agent.directChatPage.page.isExisting()) {
		await agent.directChatPage.back.click();
		await agent.homePage.ready();
	}
	await agent.homePage.openChat(contactName);
	await agent.directChatPage.ready();
}
