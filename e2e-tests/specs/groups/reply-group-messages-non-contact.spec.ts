import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

// Over the 100 processed ops the node buffers ahead of the frontend, so the
// burst holds the introduction back past the frontend's first lookup.
const BURST_SIZE = 150;

interface Member {
	name: string;
	agent: Agent;
}

async function deviceId(agent: Agent): Promise<string> {
	return agent.executeAsync((done: (id: string) => void) => {
		void window.__test.myDeviceId().then(done);
	});
}

describe('Reply quotes of a non-contact member met during catch-up', function () {
	// Every agent works through the burst's backlog at the e2e build's
	// notification pace before the reply reaches it.
	this.timeout(900_000);

	let viewer: Member;
	let introducer: Member;
	let quoted: Member;

	before(async function () {
		// The viewer catches up on the whole burst at once, which stalls a
		// phone's stores past the stalled-store alarm; the field report came
		// from a desktop.
		const [ann, ben, cat] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Ann: ann, Ben: ben, Cat: cat });

		// A catch-up from the mailbox is processed author by author in device
		// key order. The quoted member's log must come before the log carrying
		// their introduction, so the introducer is whichever sorts last.
		viewer = { name: 'Ann', agent: ann };
		const [first, second] = [
			{ name: 'Ben', agent: ben, id: await deviceId(ben) },
			{ name: 'Cat', agent: cat, id: await deviceId(cat) },
		].sort((a, b) => (a.id < b.id ? -1 : 1));
		quoted = first;
		introducer = second;

		// The viewer and the quoted member only know the introducer.
		await exchangeContacts([introducer.agent, quoted.agent]);
		await backToHome([introducer.agent, quoted.agent]);
		await exchangeContacts([introducer.agent, viewer.agent]);
		await backToHome([introducer.agent, viewer.agent]);

		await createGroup(introducer.agent, 'mygroup', [viewer.name]);
		await viewer.agent.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
	});

	it("names the quoted non-contact's author after catching up on launch", async () => {
		// The viewer meets the quoted member in one catch-up on relaunch. A
		// burst in the quoted member's log keeps its node busy between the
		// quoted member's first op and the introduction, which sits in the
		// introducer's later log.
		await viewer.agent.stopApp();

		await introducer.agent.groupChatPage.infoLink.click();
		await introducer.agent.groupInfoPage.ready();
		await introducer.agent.groupInfoPage.addMembersLink.click();
		await introducer.agent.addMembersPage.ready();
		await introducer.agent.addMembersPage.addContactByName(quoted.name);
		await introducer.agent.addMembersPage.addButton.click();
		await introducer.agent.groupInfoPage.ready();
		await introducer.agent.groupInfoPage.back.click();
		await introducer.agent.groupChatPage.ready();

		await quoted.agent.homePage
			.chatListItem('mygroup')
			.waitForExist({ timeout: SYNC_TIMEOUT });
		await quoted.agent.homePage.chatListItem('mygroup').click();
		await quoted.agent.groupChatPage.ready();
		await quoted.agent.groupChatPage.composer.sendMessage(
			'Anyone up for lunch?',
		);

		const target = await introducer.agent.groupChatPage.messages.waitForMessage(
			'Anyone up for lunch?',
		);
		await target.reply('Count me in');
		await introducer.agent.groupChatPage.messages.waitForMessage('Count me in');

		for (let i = 0; i < BURST_SIZE; i++) {
			await quoted.agent.groupChatPage.composer.sendMessage(`burst ${i}`);
		}

		await viewer.agent.startApp();
		await viewer.agent.homePage.chatListItem('mygroup').click();
		await viewer.agent.groupChatPage.ready();

		const reply =
			await viewer.agent.groupChatPage.messages.waitForMessage('Count me in');
		await reply.waitForReplyQuote('Anyone up for lunch?');
		await reply.waitForReplyQuoteAuthor(quoted.name);
	});
});
