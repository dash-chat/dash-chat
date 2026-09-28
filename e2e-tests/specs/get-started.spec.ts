import { backToHome } from '../helpers/flows/back-to-home';
import { createProfiles } from '../helpers/flows/create-profiles';
import { contactLinkOf } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';
import { waitForTestUtils } from '../setup/webview';

describe('Get Started cards', () => {
	let agent: Agent;
	let peer: Agent;

	before(async function () {
		[agent, peer] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: agent, Bob: peer });
	});

	it('shows Get Started cards on empty home', async () => {
		await agent.waitUntil(
			async () => (await agent.homePage.visibleGetStartedCards()).length > 0,
		);

		const cards = await agent.homePage.visibleGetStartedCards();
		expect(cards).toContain('add-contact');
		expect(cards).toContain('add-photo');
		expect(cards).toContain('chat-color');
	});

	it('keeps the add-contact card after adding a contact that has not accepted yet', async () => {
		const bobsLink = await contactLinkOf(peer);

		await agent.homePage.getStartedCard('add-contact').click();
		await agent.addContactPage.ready();
		await agent.addContactPage.enterAddContactLink(bobsLink);
		await agent.directChatPage.ready();
		await backToHome([agent]);

		await agent.homePage.chatListItem('Bob').waitForExist();
		await agent.homePage.getStartedCard('add-contact').waitForExist();
	});

	it('dismisses a card and it persists after reload', async () => {
		await agent.homePage.dismissGetStartedCardButton('add-contact').click();

		await agent.homePage
			.getStartedCard('add-contact')
			.waitForExist({ reverse: true });

		await agent.execute(() => window.location.reload());
		await waitForTestUtils(agent);
		await agent.homePage.ready();

		const cards = await agent.homePage.visibleGetStartedCards();
		expect(cards).not.toContain('add-contact');
		expect(cards).toContain('add-photo');
	});
});
