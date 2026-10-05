/**
 * The cloud mailbox over networks as slow as the ones measured across the
 * global majority, where connection setup alone takes seconds. The mailbox
 * answers every request eventually, so the chip must stay hidden and a message
 * must reach the mailbox. One agent in a solo group, so the chip is on screen.
 */
import { formatStatusTrace } from '../helpers/components/connection-status-indicator';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { NETWORK_PROFILES } from '../helpers/network-profiles';
import { MAILBOX_HUNG_MS, UI_TIMEOUT } from '../helpers/timeouts';
import {
	healMailboxLink,
	isRemoteMailbox,
	shapeMailboxLink,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Cloud mailbox over slow networks', function () {
	this.timeout(300_000);

	let agent: Agent;
	let linkOpened = false;

	const chip = () => agent.groupChatPage.connectionStatusIndicator;
	const messages = () => agent.groupChatPage.messages;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		linkOpened = true;
		await agent.createProfilePage.createProfile('Alice', 'Slow');
		await createGroup(agent, 'Solo Group', []);
		await chip().waitForStatus(
			'connected',
			UI_TIMEOUT,
			'the chip did not hide once the chat opened',
		);
	});

	afterEach(async () => {
		if (linkOpened) await healMailboxLink();
	});

	for (const [network, conditions] of Object.entries(NETWORK_PROFILES)) {
		it(`stays connected and hands over a message over ${network}`, async () => {
			const token = await chip().startRecordingStatus();
			await shapeMailboxLink(conditions);
			const text = `over ${network}`;
			await agent.groupChatPage.composer.sendMessage(text);
			await messages().waitForMessageStatus(text, ['mailbox']);
			// As long as a mailbox that never answers takes to flip the chip, so
			// requests failing at the full budget would have shown by now.
			await agent.pause(MAILBOX_HUNG_MS);
			const rendered = await chip().recordedStatuses(token);
			if (rendered.some(sample => sample.status === 'disconnected')) {
				throw new Error(
					`the chip read disconnected over ${network}, whose mailbox answers ` +
						`every request (rendered: ${formatStatusTrace(rendered)})`,
				);
			}
		});
	}
});
