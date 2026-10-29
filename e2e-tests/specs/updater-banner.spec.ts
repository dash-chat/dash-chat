import {
	type Agent,
	setupAgents,
	setupDesktopAgentWithEnv,
} from '../setup/setup-agents';
import {
	type UpdaterServer,
	startUpdaterServer,
} from '../setup/updater-server';

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

/** The version the app is told it runs, and the newer one the mock update
 *  endpoint advertises. Arbitrary, as long as the believed one is older. */
const BELIEVED_VERSION = '0.1.0';
const ADVERTISED_VERSION = '0.2.0';

describe('UpdaterBanner real update check', () => {
	let agent: Agent;
	let server: UpdaterServer;

	before(async function () {
		server = await startUpdaterServer(ADVERTISED_VERSION);
		agent = await setupDesktopAgentWithEnv({
			E2E_UPDATER_URL: server.url,
			E2E_UPDATER_CURRENT_VERSION: BELIEVED_VERSION,
		});
		await agent.createProfilePage.createProfile('Updater', 'Real');
	});

	after(async () => {
		await server?.close();
	});

	it('shows the available banner when the endpoint advertises a newer version', async () => {
		await agent.updaterBanner.banner.waitForExist();
		expect(await agent.updaterBanner.title.getText()).toBe(
			await agent.tr('updateAvailable'),
		);
	});
});
