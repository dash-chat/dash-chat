/**
 * The cloud mailbox on a spotty link: the server keeps running while the
 * link to it turns slow, hangs, or refuses connections, the way a mailbox
 * behind a struggling connection does. The chip may only stay hidden while
 * the mailbox actually answers, and a message the mailbox cannot take stays
 * pending until it can. One agent in a solo group, so the chip is on screen.
 */
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import {
	MAILBOX_HEALED_MS,
	MAILBOX_HUNG_MS,
	MAILBOX_UNANSWERED_MS,
	UI_TIMEOUT,
} from '../helpers/timeouts';
import { isRemoteMailbox, mailboxLink } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';
import type { Link } from '../setup/toxiproxy';

/** Long enough for a poll to have gone through the slow link. */
const SLOW_SETTLE_MS = 5_000;
/** Long enough for a message to have reached a healthy mailbox. */
const HELD_BACK_MS = 10_000;

describe('Cloud mailbox on a spotty link', function () {
	this.timeout(300_000);

	let agent: Agent;
	let link: Link;

	const chip = () => agent.groupChatPage.connectionStatusIndicator;
	const messages = () => agent.groupChatPage.messages;

	async function expectConnected(after: string, within: number): Promise<void> {
		await chip().waitForStatus(
			'connected',
			within,
			`the chip did not hide within ${within / 1_000}s after ${after}`,
		);
	}

	async function expectDisconnected(
		after: string,
		within: number,
	): Promise<void> {
		await chip().waitForStatus(
			'disconnected',
			within,
			`the chip did not read disconnected within ${within / 1_000}s after ${after}`,
		);
	}

	/** Send `text` and check the mailbox does not get to hold it. */
	async function sendHeldBack(text: string): Promise<void> {
		await agent.groupChatPage.composer.sendMessage(text);
		await agent.pause(HELD_BACK_MS);
		expect(await messages().messageStatusFor(text)).not.toBe('mailbox');
	}

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		link = mailboxLink();
		await agent.createProfilePage.createProfile('Alice', 'Spotty');
		await createGroup(agent, 'Solo Group', []);
		await expectConnected('the chat opened', UI_TIMEOUT);
	});

	after(async () => {
		if (link !== undefined) await link.heal();
	});

	it('stays connected and hands over a message over a slow link', async () => {
		await link.slow();
		await agent.pause(SLOW_SETTLE_MS);
		expect(await chip().status()).toBe('connected');
		await agent.groupChatPage.composer.sendMessage('slowly does it');
		await messages().waitForMessageStatus('slowly does it', ['mailbox']);
	});

	it('reads disconnected while the link hangs, holds the message back, and hands it over once the link heals', async () => {
		await link.hang();
		await expectDisconnected('the link hung', MAILBOX_HUNG_MS);
		await sendHeldBack('while you were hanging');
		await link.heal();
		await expectConnected('the link healed', MAILBOX_HEALED_MS);
		await messages().waitForMessageStatus('while you were hanging', [
			'mailbox',
		]);
	});

	it('reads disconnected while connections are refused, holds the message back, and hands it over once they are accepted again', async () => {
		await link.cut();
		await expectDisconnected('the link was cut', MAILBOX_UNANSWERED_MS);
		await sendHeldBack('while you were refusing');
		await link.heal();
		await expectConnected('the link healed', MAILBOX_HEALED_MS);
		await messages().waitForMessageStatus('while you were refusing', [
			'mailbox',
		]);
	});
});
