import { type Agent, setupAgents } from '../setup/setup-agents';

describe('UpdaterBanner', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, ['desktop']);
		await agent.createProfilePage.createProfile('Updater', 'Test');
	});

	afterEach(async () => {
		await agent.updaterBanner.simulateUpdate('hidden');
		await agent.updaterBanner.banner.waitForExist({ reverse: true });
	});

	it('shows available banner with update message', async () => {
		await agent.updaterBanner.simulateUpdate('available');
		await agent.updaterBanner.banner.waitForExist();
		expect(await agent.updaterBanner.title.getText()).toBeTruthy();
	});

	it('shows downloading banner with progress bar', async () => {
		await agent.updaterBanner.simulateUpdate('downloading');
		await agent.updaterBanner.banner.waitForExist();
	});

	it('shows ready banner', async () => {
		await agent.updaterBanner.simulateUpdate('ready');
		await agent.updaterBanner.banner.waitForExist();
	});

	it('shows error banner', async () => {
		await agent.updaterBanner.simulateUpdate('error');
		await agent.updaterBanner.banner.waitForExist();
	});

	it('dismisses banner when X is clicked', async () => {
		await agent.updaterBanner.simulateUpdate('available');
		await agent.updaterBanner.banner.waitForExist();

		await agent.updaterBanner.dismiss();
		await agent.updaterBanner.banner.waitForExist({ reverse: true });
	});
});
