/**
 * Chat performance under a flooded history: a single agent seeds a solo group
 * with thousands of messages through the real send_message command, then
 * checks the chat still opens, scrolls, and sends within interactive budgets.
 *
 * Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=desktop just test e2e message-list-performance
 *
 * E2E_MESSAGE_COUNT sets how many messages are seeded (default 1000).
 */
import { type Agent, setupAgents } from '../setup/setup-agents';

const MESSAGE_COUNT = Number(process.env.E2E_MESSAGE_COUNT ?? '1000');
if (!Number.isFinite(MESSAGE_COUNT) || MESSAGE_COUNT <= 0) {
	throw new Error(
		`Invalid E2E_MESSAGE_COUNT: ${process.env.E2E_MESSAGE_COUNT}`,
	);
}
const SEED_BATCH_SIZE = 50;
const GROUP_NAME = 'Flood Test';

// The budget encodes the product bar: a chat must stay effectively instant no
// matter how long its history is — opening, scrolling, and sending alike.
// Every measurement runs in-page with performance.now() (via window.__test
// helpers) because 100ms sits far below WebDriver round-trip noise.
const BUDGET_MS = 100;

describe('Message list performance', function () {
	this.timeout(600_000);

	let agent1: Agent;
	let chatId: string;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		[agent1] = await setupAgents(this, [{ platform: 'any' }]);
		await agent1.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
	});

	it('creates a solo group to flood', async () => {
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();
		await agent1.newMessagePage.newGroup.click();

		await agent1.newGroupPage.addMembersStep.ready();
		await agent1.newGroupPage.addMembersStep.nextButton.click();

		await agent1.newGroupPage.groupInfoStep.ready();
		await agent1.newGroupPage.groupInfoStep.setName(GROUP_NAME);
		await agent1.newGroupPage.groupInfoStep.createButton.click();

		await agent1.groupChatPage.ready();
		chatId = decodeURIComponent(
			new URL(await agent1.getUrl()).pathname.split('/')[2],
		);
		expect(chatId).toBeTruthy();

		await agent1.groupChatPage.back.click();
		await agent1.homePage.ready();
	});

	it(`seeds the chat with ${MESSAGE_COUNT} messages`, async () => {
		// Seed while parked on the settings page so no mounted chat surface
		// re-renders on every one of the thousands of operations.
		await agent1.homePage.settingsLink.click();
		await agent1.settingsPage.ready();

		const startedAt = Date.now();
		for (let sent = 0; sent < MESSAGE_COUNT; sent += SEED_BATCH_SIZE) {
			const texts = Array.from(
				{ length: Math.min(SEED_BATCH_SIZE, MESSAGE_COUNT - sent) },
				(_, i) => `flood ${sent + i}`,
			);
			await agent1.execute(
				async (id: string, batch: string[]) =>
					window.__test.sendMessages(id, batch),
				chatId,
				texts,
			);
			if ((sent + SEED_BATCH_SIZE) % 500 === 0) {
				console.log(
					`[flood] seeded ${sent + SEED_BATCH_SIZE}/${MESSAGE_COUNT} in ${Date.now() - startedAt}ms`,
				);
			}
		}
		console.log(
			`[flood] seeded ${MESSAGE_COUNT} messages in ${Date.now() - startedAt}ms`,
		);

		await agent1.settingsPage.back.click();
		await agent1.homePage.ready();
	});

	it(`opens the flooded chat within ${BUDGET_MS}ms`, async () => {
		const openMs = await agent1.homePage.openGroupChatRenderMs(
			GROUP_NAME,
			MESSAGE_COUNT,
		);
		console.log(`[flood] chat opened and rendered in ${openMs}ms`);

		expect(await agent1.groupChatPage.messages.renderedCount()).toEqual(
			MESSAGE_COUNT,
		);
		expect(openMs).toBeLessThanOrEqual(BUDGET_MS);
	});

	it('scrolls the flooded history responsively', async () => {
		const scroll = agent1.groupChatPage.scroll;

		const toTopMs = await scroll.jumpMs('top');
		// Two animation frames right after the jump: how long the main thread
		// stays busy before the app can paint again.
		const frameMs = await agent1.execute(() => window.__test.frameGapMs());
		const toBottomMs = await scroll.jumpMs('bottom');
		await agent1.waitUntil(() => scroll.isAtBottom());

		console.log(
			`[flood] scrollToTop ${toTopMs}ms, frame ${frameMs}ms, scrollToBottom ${toBottomMs}ms`,
		);
		expect(toTopMs).toBeLessThanOrEqual(BUDGET_MS);
		expect(frameMs).toBeLessThanOrEqual(BUDGET_MS);
		expect(toBottomMs).toBeLessThanOrEqual(BUDGET_MS);
	});

	it(`renders a newly sent message within ${BUDGET_MS}ms`, async () => {
		await agent1.groupChatPage.composer.type('after the flood');
		const sendMs =
			await agent1.groupChatPage.messages.sendRenderMs('after the flood');
		console.log(`[flood] send-to-render took ${sendMs}ms`);
		expect(sendMs).toBeLessThanOrEqual(BUDGET_MS);
	});
});
