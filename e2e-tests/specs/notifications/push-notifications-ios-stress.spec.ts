import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/** Rounds of traffic the iPhone takes with its app open: every round wakes
 *  a fresh push extension beside the running app. */
const ROUNDS = 3;
/** Messages each side sends in a round, interleaved with the other's. Every
 *  message the Mac sends is a push to the iPhone, so its extension ingests a
 *  backlog while the app writes the same database. */
const BURST = 20;

/**
 * Traffic that lands while the app is open must leave the app whole: every
 * message sent arrives, every message received is kept, and groups can
 * still be created and joined. On iOS the push extension runs a node on the
 * app's data under the app's key, beside the app's own node: the relay drops
 * the app's session for the duplicate id, and the two processes contend for
 * the SQLite file, so a send fails or an incoming operation is dropped for
 * good. The collision is local to the iPhone, so the Mac is only here to push
 * traffic at it — one iPhone is enough to reproduce.
 *
 * Skips itself unless E2E_STRESS=1, and unless push testing is available (a
 * Firebase service-account key plus a mobile agent). Run it with:
 *   PLATFORMS=ios,desktop just e2e run notifications/push-notifications-ios-stress
 */
// wdio arms its per-test abort timer from the mocha timeout at invocation
// time, so `this.timeout()` inside the test body comes too late — it must be
// set suite-wide. `ROUNDS * BURST * 2` sends on a phone take well past the
// 300s default, and each one gets slower as the chat grows.
describe('Traffic landing while the app is open', function () {
	this.timeout(1_800_000);

	let iphone: Agent;
	let mac: Agent;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (!pushTestingEnabled()) this.skip();
		[iphone, mac] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'desktop' },
		]);
		await iphone.enablePreviewFeatures();
		await mac.enablePreviewFeatures();
		await iphone.createProfilePage.createProfile('Rex', 'Test');
		await mac.createProfilePage.createProfile('Sam', 'Test');
	});

	/** Both sides send `BURST` messages, turn and turn about, then each
	 *  must show every message of both. */
	async function crossfire(round: number): Promise<void> {
		const fromRex = Array.from(
			{ length: BURST },
			(_, i) => `rex ${round}.${i + 1}`,
		);
		const fromSam = Array.from(
			{ length: BURST },
			(_, i) => `sam ${round}.${i + 1}`,
		);
		for (let i = 0; i < BURST; i++) {
			await iphone.directChatPage.composer.sendMessage(fromRex[i]);
			await mac.directChatPage.composer.sendMessage(fromSam[i]);
		}
		for (const message of [...fromRex, ...fromSam]) {
			await iphone.directChatPage.messages.waitForMessage(message);
			await mac.directChatPage.messages.waitForMessage(message);
		}
	}

	/** `creator`, on its home page, makes a group with `member`; `other` must
	 *  list it. Both end on their home pages. */
	async function groupSeenByOther(
		creator: Agent,
		other: Agent,
		name: string,
		member: string,
	): Promise<void> {
		await createGroup(creator, name, [member]);
		await creator.groupChatPage.back.click();
		await creator.homePage.ready();
		await other.homePage
			.chatListItem(name)
			.waitForExist({ timeout: SYNC_TIMEOUT });
	}

	it('leaves both apps keeping every message and joining groups', async () => {
		// The apps have to be settled on the relay before the first push: a
		// node only seconds old has no relay session for the extension to take.
		await iphone.pause(15_000);
		// A fresh extension process builds its node from the current data; one
		// left over from an earlier push holds a node under an older identity,
		// which collides with nothing. iOS evicts extensions on its own
		// schedule, so a push meets a fresh one often enough in real use.
		await iphone.killPushExtension();
		// The contact requests are pushes the iPhone gets with the app open.
		await exchangeContacts([iphone, mac]);

		for (let round = 1; round <= ROUNDS; round++) {
			await iphone.killPushExtension();
			await crossfire(round);
		}

		await iphone.directChatPage.back.click();
		await iphone.homePage.ready();
		await mac.directChatPage.back.click();
		await mac.homePage.ready();
		await groupSeenByOther(mac, iphone, 'group from sam', 'Rex');
		await groupSeenByOther(iphone, mac, 'group from rex', 'Sam');
	});
});
