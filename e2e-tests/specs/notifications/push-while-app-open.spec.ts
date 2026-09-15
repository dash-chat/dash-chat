import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/** Rounds of traffic each phone takes with its app open: every round wakes
 *  a fresh push extension beside each running app. */
const ROUNDS = 3;
/** Messages each phone sends in a round, interleaved with the other's. Every
 *  one is a push, so both extensions ingest a backlog while both apps write
 *  the same database. */
const BURST = 20;

/**
 * Traffic that lands while the app is open must leave the app whole: every
 * message sent arrives, every message received is kept, and groups can
 * still be created and joined. On iOS the push extension runs a node on the
 * app's data under the app's key, beside the app's own node: the relay drops
 * the app's session for the duplicate id, and the two processes contend for
 * the SQLite file, so a send fails or an incoming operation is dropped for
 * good. Only runs when `E2E_PUSH=1`.
 */
describe('Traffic landing while the app is open', () => {
	let phone1: Agent;
	let phone2: Agent;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[phone1, phone2] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'ios' },
		]);
		await phone1.enablePreviewFeatures();
		await phone2.enablePreviewFeatures();
		await phone1.createProfilePage.createProfile('Rex', 'Test');
		await phone2.createProfilePage.createProfile('Sam', 'Test');
	});

	/** A fresh extension process builds its node from the current data; one
	 *  left over from an earlier push holds a node under an older identity,
	 *  which collides with nothing. iOS evicts extensions on its own schedule,
	 *  so a push meets a fresh one often enough in real use. */
	async function evictPushExtensions(): Promise<void> {
		await phone1.killPushExtension();
		await phone2.killPushExtension();
	}

	/** Both phones send `BURST` messages, turn and turn about, then each
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
			await phone1.directChatPage.composer.sendMessageOnce(fromRex[i]);
			await phone2.directChatPage.composer.sendMessageOnce(fromSam[i]);
		}
		for (const message of [...fromRex, ...fromSam]) {
			await phone1.directChatPage.messages.waitForMessage(message);
			await phone2.directChatPage.messages.waitForMessage(message);
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
		await phone1.pause(15_000);
		await evictPushExtensions();
		// The contact requests are pushes each phone gets with the app open.
		await exchangeContacts(phone1, phone2);

		for (let round = 1; round <= ROUNDS; round++) {
			await evictPushExtensions();
			await crossfire(round);
		}

		await phone1.directChatPage.back.click();
		await phone1.homePage.ready();
		await phone2.directChatPage.back.click();
		await phone2.homePage.ready();
		await groupSeenByOther(phone2, phone1, 'group from sam', 'Rex');
		await groupSeenByOther(phone1, phone2, 'group from rex', 'Sam');
	});
});
