/**
 * Review checks E2E test.
 *
 * Drives the agent through every page across all theme × layout × color ×
 * locale combinations, running overflow/dark-mode/RTL checks at each stop.
 */

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	assertNoIssues,
	reloadToHome,
	switchCombo,
} from '../helpers/review/runner';
import { visitAllPages } from '../helpers/review/visit-all-pages';
import { type Agent, setLocale, setupAgents } from '../setup/setup-agents';

describe('Review checks', function () {
	this.timeout(240_000);

	let agent1: Agent;
	let agent2: Agent;
	let wideSupported: boolean;

	before(async function () {
		this.timeout(180_000);

		[agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);

		wideSupported = await agent1.supportsWideScreen();

		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Tester');

		await exchangeContacts(agent1, agent2);

		await Promise.all([
			agent1.directChatPage.composer.messageInput.waitForExist(),
			agent2.directChatPage.composer.messageInput.waitForExist(),
		]);

		await agent1.directChatPage.composer.sendMessage('Hello from Alice!');
		await agent2.directChatPage.messages.waitForMessage('Hello from Alice!');

		await agent2.directChatPage.composer.sendMessage('Hello from Bob!');
		await agent1.directChatPage.messages.waitForMessage('Hello from Bob!');

		await reloadToHome(agent1);
	});

	beforeEach(async function () {
		this.timeout(120_000);
		if (!(await agent1.homePage.isLoaded())) {
			await reloadToHome(agent1);
		}
	});

	describe('English - Light', function () {
		it('Material Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'material', true);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('Material Mobile', async () => {
			await switchCombo(agent1, 'material', false);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('iOS Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'ios', true);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('iOS Mobile', async () => {
			await switchCombo(agent1, 'ios', false);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});
	});

	describe('English - Dark', function () {
		it('Material Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'material', true, true);
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkDarkMode: true }),
			);
		});

		it('Material Mobile', async () => {
			await switchCombo(agent1, 'material', false, true);
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkDarkMode: true }),
			);
		});

		it('iOS Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'ios', true, true);
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkDarkMode: true }),
			);
		});

		it('iOS Mobile', async () => {
			await switchCombo(agent1, 'ios', false, true);
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkDarkMode: true }),
			);
		});
	});

	describe('German (de-de)', function () {
		before(async function () {
			this.timeout(60_000);
			await reloadToHome(agent1);
			await setLocale(agent1, 'de-de');
			await agent1.homePage.ready();
		});

		it('Material Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'material', true);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('Material Mobile', async () => {
			await switchCombo(agent1, 'material', false);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('iOS Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'ios', true);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});

		it('iOS Mobile', async () => {
			await switchCombo(agent1, 'ios', false);
			assertNoIssues(await visitAllPages(agent1, { hasChat: true }));
		});
	});

	describe('Farsi RTL (fa-ir)', function () {
		before(async function () {
			this.timeout(60_000);
			await reloadToHome(agent1);
			await setLocale(agent1, 'fa-ir');
			await agent1.homePage.ready();
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
		});

		it('Material Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'material', true);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkRTL: true }),
			);
		});

		it('Material Mobile', async () => {
			await switchCombo(agent1, 'material', false);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkRTL: true }),
			);
		});

		it('iOS Desktop', async function () {
			if (!wideSupported) this.skip();
			await switchCombo(agent1, 'ios', true);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkRTL: true }),
			);
		});

		it('iOS Mobile', async () => {
			await switchCombo(agent1, 'ios', false);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			assertNoIssues(
				await visitAllPages(agent1, { hasChat: true, checkRTL: true }),
			);
		});
	});
});
