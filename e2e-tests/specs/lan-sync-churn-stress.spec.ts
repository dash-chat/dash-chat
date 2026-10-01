/**
 * Two agents on one LAN keep syncing peer-to-peer while their address books
 * fill up with contacts who come and go.
 *
 * Every round a new desktop agent is launched, becomes a contact of both main
 * agents over the cloud mailbox (which puts it in their address books, as every
 * mailbox author is in production), and is killed again — one at a time, so
 * the machine never runs more than one extra app. Then the mailbox link is cut
 * and the main agents must still deliver to each other both ways.
 *
 * The main agents are whatever PLATFORMS provides (desktop by default, phones
 * if asked for); the short-lived ones are always desktop.
 *
 * Skips itself unless E2E_STRESS=1, and against a remote mailbox, whose link
 * can't be cut. Run it with:
 *   E2E_STRESS=1 just e2e run lan-sync-churn-stress
 *
 * Tunables: E2E_CHURN_MINUTES (how long to keep churning, default 20).
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	type NamedAgent,
	expectSyncBothWays,
	meetVisitor,
	withMailboxCut,
} from '../helpers/flows/lan-sync';
import { envInt } from '../helpers/utils';
import { mailboxDegradable } from '../setup/mailbox-control';
import { setupAgents } from '../setup/setup-agents';

const CHURN_MS = envInt('E2E_CHURN_MINUTES', 20) * 60 * 1_000;

/** Room for the round that is still running when the churn time is up. */
const LAST_ROUND_MS = 10 * 60 * 1_000;

// wdio arms its per-hook abort timer from the mocha timeout at invocation time,
// so this has to be set suite-wide rather than inside a hook or test body.
describe('LAN sync under contact churn', function () {
	this.timeout(CHURN_MS + LAST_ROUND_MS);

	let alice: NamedAgent;
	let bob: NamedAgent;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (!mailboxDegradable()) this.skip();
		const [agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		alice = { agent: agent1, name: 'Alice' };
		bob = { agent: agent2, name: 'Bob' };
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
	});

	it('keeps the main agents syncing while contacts come and go', async () => {
		const startedAt = Date.now();
		for (let round = 1; Date.now() - startedAt < CHURN_MS; round++) {
			await meetVisitor(round, [alice, bob]);
			await withMailboxCut(() => expectSyncBothWays(alice, bob, `${round}`));
			console.log(
				`[lan-sync-churn] round ${round} synced after ${Math.round((Date.now() - startedAt) / 1_000)}s`,
			);
		}
	});
});
