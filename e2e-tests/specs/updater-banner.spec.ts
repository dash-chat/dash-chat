import {
	type Agent,
	setupAgents,
	setupDesktopAgentWithEnv,
} from '../setup/setup-agents';
import {
	type UpdaterServer,
	productionEndpointReachable,
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

/** What the mock endpoint advertises: far enough ahead to stay newer than any
 *  real build version, so the plugin's own comparison finds an update — and,
 *  for the negative case, far enough behind any. */
const ADVERTISED_VERSION = '999.0.0';
const OLDER_VERSION = '0.0.1';

describe('UpdaterBanner real check against a mock endpoint', () => {
	let agent: Agent;
	let server: UpdaterServer;

	before(async function () {
		server = await startUpdaterServer(ADVERTISED_VERSION);
		agent = await setupDesktopAgentWithEnv({ E2E_UPDATER_URL: server.url });
		await agent.createProfilePage.createProfile('Updater', 'Mock');
	});

	after(async () => {
		await server?.close();
	});

	// The following test's leading no-banner assertion doubles as this one's
	// proof: a banner the older version wrongly produced would fail it.
	it('keeps the banner hidden when the endpoint advertises an older version', async () => {
		server.setVersion(OLDER_VERSION);
		await agent.updaterBanner.checkForUpdate();
		// Let the check against localhost land before asserting nothing showed.
		await agent.pause(1_000);
		await expect(agent.updaterBanner.banner).not.toExist();
	});

	it('shows the available banner when the endpoint advertises a newer version', async () => {
		// Attributes the banner to the explicit check below — and fails loudly
		// if e2e builds ever start checking on mount, at which point the 'check'
		// trigger chain is dead code (see UpdaterBanner.checkForUpdate).
		await expect(agent.updaterBanner.banner).not.toExist();
		server.setVersion(ADVERTISED_VERSION);
		await agent.updaterBanner.checkForUpdate();
		await agent.updaterBanner.banner.waitForExist();
		expect(await agent.updaterBanner.title.getText()).toBe(
			await agent.tr('updateAvailable'),
		);
	});
});

/** What the app is told it runs against the production endpoint: below any
 *  published release, so the latest one counts as an update. */
const BELIEVED_VERSION = '0.0.1';

describe('UpdaterBanner real check against the production endpoint', () => {
	let agent: Agent;

	before(async function () {
		if (!(await productionEndpointReachable())) {
			console.log(
				'[updater-banner] production update endpoint unreachable — skipping',
			);
			this.skip();
		}
		agent = await setupDesktopAgentWithEnv({
			E2E_UPDATER_CURRENT_VERSION: BELIEVED_VERSION,
		});
		await agent.createProfilePage.createProfile('Updater', 'Real');
	});

	it('shows the available banner for the latest published release', async () => {
		await expect(agent.updaterBanner.banner).not.toExist();
		await agent.updaterBanner.checkForUpdate();
		// The harness could reach the endpoint (the skip probe above), so a
		// miss here means the app's own fetch of it failed or its manifest
		// no longer parses.
		await agent.updaterBanner.banner.waitForExist({
			timeoutMsg:
				'no update banner: the app could not fetch or accept the production latest.json',
		});
		expect(await agent.updaterBanner.title.getText()).toBe(
			await agent.tr('updateAvailable'),
		);
	});
});
