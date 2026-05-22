/**
 * Full messaging flow E2E test.
 *
 * Uses two Tauri instances (agent1 & agent2) via WebdriverIO multiremote.
 * Calls window.__test functions registered by ui/tests/setup-utils.ts.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	type Agent,
	setupAgent,
	waitForTestUtils,
} from '../helpers/setup-agents';

describe('Full messaging flow', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('creates profiles on both agents', async () => {
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
	});

	it('shows Get Started cards on empty home', async () => {
		await agent1.waitUntil(
			async () => (await agent1.homePage.visibleGetStartedCards()).length > 0,
			{ timeout: 10_000, timeoutMsg: 'No Get Started cards visible' },
		);

		const cards = await agent1.homePage.visibleGetStartedCards();
		expect(cards).toContain('add-contact');
		expect(cards).toContain('add-photo');
		expect(cards).toContain('chat-color');
	});

	it('dismisses a Get Started card and it persists after reload', async () => {
		await agent1.homePage.dismissGetStartedCardButton('add-contact').click();

		await agent1.homePage.getStartedCard('add-contact').waitForExist({
			reverse: true,
			timeout: 5_000,
			timeoutMsg: 'Add contact card still visible after dismiss',
		});

		// Reload and verify dismissal persists
		await agent1.execute(() => window.location.reload());
		await waitForTestUtils(agent1);
		await agent1.homePage.ready();

		const cards = await agent1.homePage.visibleGetStartedCards();
		expect(cards).not.toContain('add-contact');
		expect(cards).toContain('add-photo');
	});

	it('exchanges contact codes between agents', async () => {
		await exchangeContacts(agent1, agent2);
	});

	it('sends a message from Alice to Bob', async () => {
		await agent1.directChatPage.sendMessage('Hello from Alice!');
		await agent1.directChatPage.waitForMessage('Hello from Alice!');
		await agent2.directChatPage.waitForMessage('Hello from Alice!');
	});

	it('sends a reply from Bob to Alice', async () => {
		await agent2.directChatPage.sendMessage('Hello from Bob!');
		await agent2.directChatPage.waitForMessage('Hello from Bob!');
		await agent1.directChatPage.waitForMessage('Hello from Bob!');
	});

	// it('displays the app version on the help page', async () => {
	// 	await agent1.goto('/settings/help');
	// 	await agent1.waitUntil(
	// 		async () => agent1.execute(() => window.__test.versionItem() !== null),
	// 		{ timeout: 10_000, timeoutMsg: 'Version item not visible on help page' },
	// 	);

	// 	const versionText = await agent1.execute(
	// 		() => (window.__test.versionItem() as HTMLElement)?.textContent ?? '',
	// 	);
	// 	expect(versionText).toMatch(/\d+\.\d+\.\d+/);
	// });
});
