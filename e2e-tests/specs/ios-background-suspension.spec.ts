/**
 * The iPhone app survives being left right after a message arrives. Reading a
 * message makes the app write (the message, then its delivery acknowledgement),
 * and iOS kills (0xdead10cc) an app it suspends mid-write on a database in the
 * App Group container. Leaving after slightly different delays sweeps the
 * moment of suspension across those writes.
 *
 * A write is open for only milliseconds, so a single round rarely lands inside
 * one: it takes many. Skips itself unless E2E_STRESS=1:
 *   E2E_STRESS=1 PHONES=ios just e2e run ios-background-suspension
 *
 * Tunables: E2E_SUSPENSION_ROUNDS (default 42).
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

const ROUNDS = Number(process.env.E2E_SUSPENSION_ROUNDS ?? 42);
/** How long after seeing the message the user leaves, cycled over the rounds. */
const LEAVE_AFTER_MS = [0, 100, 200, 300, 400, 500, 600];
/** Long enough for iOS to have suspended the app whatever the pause does. */
const AWAY_MS = 5_000;

describe('iPhone app left right after a message arrives', function () {
	this.timeout(1_800_000);

	let alice: Agent;
	let bob: Agent;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'ios' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
		await alice.directChatPage.composer.sendMessage('before the rounds');
		await bob.directChatPage.messages.waitForMessage('before the rounds');
	});

	for (let round = 1; round <= ROUNDS; round++) {
		const leaveAfterMs = LEAVE_AFTER_MS[(round - 1) % LEAVE_AFTER_MS.length];
		it(`round ${round}: is not killed when left ${leaveAfterMs}ms after a message`, async () => {
			await alice.directChatPage.composer.sendMessage(`round ${round}`);
			await bob.directChatPage.messages.waitForMessage(`round ${round}`);
			await bob.pause(leaveAfterMs);
			await bob.backgroundApp();
			await bob.pause(AWAY_MS);

			expect(await bob.isAppRunning()).toBe(true);
			await bob.startApp();
		});
	}
});
