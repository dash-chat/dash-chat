import {
	type Agent,
	setupAgents,
	waitForTestUtils,
} from '../setup/setup-agents';

describe('Get Started cards', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, ['any']);
		await agent.createProfilePage.createProfile('Alice', 'Test');
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
