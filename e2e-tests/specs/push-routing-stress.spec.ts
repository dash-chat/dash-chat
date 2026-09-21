/**
 * Adding contacts, at scale, on a phone: eight desktop agents and one Android
 * phone add each other, message, read and leave the app, and every step is
 * checked — the chat each add lands on, what every agent can see afterwards,
 * and, on the phone, exactly which notifications its device is showing.
 *
 * Nothing is set up behind the checks: the agents start as strangers and the
 * moves do the exchanging, so a contact request that never arrives, one that
 * notifies the wrong device, or one whose notification survives being read is
 * a failing move rather than a quietly broken fixture.
 *
 * Eight contacts is the point: with one chat any notification route gets it
 * right, and the bug this searches for is a tap that opens the chat the app
 * was last on, or the one whose notification launched it, instead of the
 * tapped one.
 *
 * Needs a physical Android receiver (its notifications are read through
 * dumpsys) and a Firebase service-account key, and skips itself unless
 * E2E_STRESS=1:
 *   PLATFORMS=android,desktop,desktop,desktop,desktop,desktop,desktop,desktop,desktop \
 *     just e2e run push-routing-stress
 *
 * Tunables: E2E_STRESS_ATTEMPTS (sequences to try, default 20),
 * E2E_STRESS_COMMANDS (moves per sequence, default 15), E2E_STRESS_SEED
 * (default random; the run logs it — re-run with the same seed to reproduce
 * a failure).
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { Fuzzer } from '../helpers/fuzz/fuzzer';
import { exchangeContactMoves } from '../helpers/fuzz/moves/contacts';
import { deviceMoves } from '../helpers/fuzz/moves/device';
import { notificationMoves } from '../helpers/fuzz/moves/notification';
import { move as textMove } from '../helpers/fuzz/moves/text-messages';
import { envInt } from '../helpers/utils';
import { mailboxWakesPhones } from '../setup/mailbox-control';
import { pushTestingEnabled } from '../setup/push-server';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** The phone's would-be contacts. Distinct names, none a substring of
 *  another: each one titles its own notifications and its own chat row. */
const CONTACTS = [
	'Alice',
	'Bob',
	'Carol',
	'Dave',
	'Erin',
	'Frank',
	'Grace',
	'Heidi',
];

const PHONE = 'Rex';

describe('Adding contacts on a phone, at scale', () => {
	let fuzzer: Fuzzer;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The phone runs the OS notification path, and its notifications are
		// read over adb.
		if (!pushTestingEnabled()) this.skip();
		const [phone, ...desktops] = await setupAgents(this, [
			{ platform: 'phone' },
			...CONTACTS.map(() => ({ platform: 'desktop' }) as const),
		]);
		const agents: Record<string, Agent> = { [PHONE]: phone };
		CONTACTS.forEach((name, i) => (agents[name] = desktops[i]));
		await createProfiles(agents);
		// Asked here because this is where the model reads it: a mailbox that
		// answered before the agents booted can miss the health probe once they
		// are all running, and the model would then route no push at all —
		// reporting every notification an away phone really gets as one it
		// cannot know.
		if (!(await mailboxWakesPhones())) {
			throw new Error(
				'the mailbox is not forwarding pushes, so push routing cannot be exercised',
			);
		}
		fuzzer = await Fuzzer.prepare(this, { agents });
	});

	it('every add, message and notification is what the agents can know', async () => {
		const attempts = envInt('E2E_STRESS_ATTEMPTS', 20);
		const length = envInt('E2E_STRESS_COMMANDS', 15);
		const seed = envInt('E2E_STRESS_SEED', Math.floor(Math.random() * 2 ** 31));
		await fuzzer.search({
			moves: [
				...exchangeContactMoves,
				...notificationMoves,
				...deviceMoves,
				// Enough messaging that there are chats worth being notified
				// about; what this spec searches is everything around requests.
				{ build: (a, c) => textMove.sendText(a, c), weight: 4 },
			],
			attempts,
			length,
			seed,
		});
	});
});
