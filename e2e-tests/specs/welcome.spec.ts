import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Welcome screen', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
	});

	it('shows the welcome screen on first launch', async () => {
		await agent.welcomePage.ready();
		expect(await agent.welcomePage.title.getText()).toBe(
			await agent.tr('welcomeTitle'),
		);
	});

	it('opens the EULA from the terms link and goes back', async () => {
		await agent.welcomePage.termsLink.click();
		await agent.eulaPage.ready();
		expect(await agent.eulaPage.body.getText()).toContain(
			'End User Licence Agreement',
		);
		await agent.eulaPage.back.click();
		await agent.welcomePage.ready();
	});

	it('goes to Set Profile on continue, focuses the name, and goes back', async () => {
		await agent.welcomePage.continueButton.click();
		await agent.createProfilePage.ready();
		expect(await agent.createProfilePage.nameInputIsFocused()).toBe(true);
		await agent.createProfilePage.back.click();
		await agent.welcomePage.ready();
	});

	it('creates a profile from Set Profile', async () => {
		await agent.createProfilePage.createProfile('Welcome', 'Test');
		await agent.homePage.ready();
	});
});
