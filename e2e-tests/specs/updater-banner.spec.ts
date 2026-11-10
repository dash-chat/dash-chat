import { type Agent, setupAgents } from '../setup/setup-agents';

describe('UpdaterBanner', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'desktop' }]);
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

	it('asks to move the app when it runs from a disk image', async () => {
		await agent.updaterBanner.simulateUpdate('misplaced');
		await agent.updaterBanner.banner.waitForExist();
		expect(await agent.updaterBanner.banner.getText()).toContain(
			await agent.tr('updateTapToMove'),
		);
	});

	it('shows the move running', async () => {
		await agent.updaterBanner.simulateUpdate('moving');
		await agent.updaterBanner.banner.waitForExist();
		expect(await agent.updaterBanner.title.getText()).toBe(
			await agent.tr('updateMoving'),
		);
	});

	// The e2e build leaves the plugin out, so the move itself fails.
	it('says when tapping to move could not move the app', async () => {
		await agent.updaterBanner.simulateUpdate('misplaced');
		await agent.updaterBanner.banner.waitForExist();
		await agent.updaterBanner.banner.click();
		await expect(agent.updaterBanner.title).toHaveText(
			await agent.tr('updateMoveError'),
		);
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
