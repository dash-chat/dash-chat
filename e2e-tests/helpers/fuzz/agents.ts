/**
 * The `Real` side of a fuzz run — the driveable agents, plus whatever
 * networks and hubs a run creates — and the UI navigation moves are built
 * from. Nothing here compares a screen with the model: that is `checks.ts`.
 */
import { type LocalHub, stopLocalHub } from '../../setup/local-hub';
import { mailboxDegradable } from '../../setup/mailbox-control';
import type { Agent } from '../../setup/setup-agents';
import type { WifiNetwork } from '../../setup/test-env';
import {
	type NotificationHelper,
	readableNotificationsFor,
} from '../components/notifications';
import { navigateToAddContact } from '../flows/exchange-contacts';
import type { DirectChatPage } from '../pages/direct-chats/direct-chat-page';
import type { GroupChatPage } from '../pages/group-chat/group-chat-page';
import { tid } from '../selectors';
import { SYNC_TIMEOUT } from '../timeouts';
import { stampedLog } from '../utils';
import {
	type ExpectedChat,
	type ExpectedModel,
	type NotificationTexts,
	PHOTO_COUNTS,
} from './model';

// Mirrors QUICK_EMOJIS in ui/src/lib/utils/emojis.ts.
export const QUICK_EMOJIS = ['❤️', '👍', '👎', '😂', '😮', '😢'];

export interface StressAgent {
	agent: Agent;
	/** The profile first name, as chat lists show it and its notifications
	 * are titled with. Must be unique across the run's agents and not a
	 * substring of another agent's name. */
	name: string;
	/** This agent's add-contact link, once preparation has collected it. */
	link: string | null;
	/** Its device's notifications, when they can be read without disturbing
	 * the app; null when the platform offers no such reading. */
	notifications: NotificationHelper | null;
	/** The wording its app puts in a notification it cannot fully name, read
	 * off the device before the run starts because a check may need it while
	 * the app is in no state to be asked. Null for an agent the run does not
	 * read. */
	notificationTexts: NotificationTexts | null;
}

/** Read the wording `sa`'s app uses for what a notification cannot name. */
export async function readNotificationTexts(
	sa: StressAgent,
): Promise<NotificationTexts | null> {
	if (sa.notifications === null) return null;
	const [generic, unnamedSender, voice] = await Promise.all([
		sa.agent.tr('youHaveANewMessage'),
		sa.agent.tr('newMessage'),
		sa.agent.tr('voiceMessage'),
	]);
	const photos: Record<number, string> = {};
	for (const count of PHOTO_COUNTS) {
		photos[count] =
			count === 1
				? await sa.agent.tr('photo')
				: await sa.agent.tr('photosCount', { count });
	}
	return { generic, unnamedSender, photos, voice };
}

/** A hub identity: its db, key and port survive its process, so bringing it
 * up on another network is the same hub moving, as a deployed one would. */
export interface HubReal {
	name: string;
	port: number;
	process: LocalHub | null;
}

/** The driveable agents, the networks a run may use, where the hubs are,
 * and the hubs the run has created so far. The fuzzer owns it so it can
 * tear down whatever a run leaves behind. */
export interface Real {
	agents: StressAgent[];
	/** The configured networks, in the order moves index them, the host's
	 * own one marked as home. */
	networks: WifiNetwork[];
	/** The host's Wi-Fi card, null when it has none or no network is
	 * configured. */
	hubsDevice: string | null;
	/** The lab SSID the card is on, null while it is on its usual network.
	 * Where every hub is today: a later slice with a card (or a machine) per
	 * hub replaces this with a location on `HubReal`. */
	hubsNetwork: string | null;
	hubs: HubReal[];
	/** Whether the cloud mailbox answered when the run started. The mailbox
	 *  module owns the link and the server's state; this is only what the
	 *  model was seeded with. */
	cloudUsable: boolean;
	/** Whether the run can degrade the link to it, which is what keeps the
	 *  cloud moves out of a run the proxy does not front. */
	cloudDegradable: boolean;
	/** Whether the run's mailbox forwards pushes: only then does anything
	 * reach a phone that is away from the foreground. */
	push: boolean;
}

/** A `Real` with no hub yet and the card off every test network. Pure: the
 * agents are driven only by preparation. */
export function newReal(init: {
	/** The run's agents by the profile name each goes by. */
	agents: Record<string, Agent>;
	networks?: WifiNetwork[];
	hubsDevice?: string | null;
	/** Whether the cloud answered when the run started; false for a spec that
	 *  took the mailbox down before preparing. */
	cloudUsable?: boolean;
	/** Whether the run's mailbox wakes a phone whose app is away, which
	 *  `Fuzzer.prepare` reads off the harness. */
	push?: boolean;
}): Real {
	const networks = init.networks ?? [];
	return {
		agents: Object.entries(init.agents).map(([name, agent]) => ({
			agent,
			name,
			link: null,
			notifications: readableNotificationsFor(agent),
			notificationTexts: null,
		})),
		networks,
		hubsDevice: networks.length === 0 ? null : (init.hubsDevice ?? null),
		hubsNetwork: null,
		hubs: [],
		cloudUsable: init.cloudUsable ?? true,
		cloudDegradable: mailboxDegradable(),
		push: init.push ?? false,
	};
}

export type ChatPage = DirectChatPage | GroupChatPage;

export function log(text: string): void {
	stampedLog(`[fuzz] ${text}`);
}

/** Resolve an abstract index against whatever options exist right now. */
export function at<T>(items: readonly T[], index: number): T {
	if (items.length === 0) throw new Error('no options to resolve against');
	return items[index % items.length];
}

export function byName(real: Real, name: string): StressAgent {
	const found = real.agents.find(a => a.name === name);
	if (found === undefined) throw new Error(`no agent named ${name}`);
	return found;
}

/** The notifications of an agent whose device a run reads. */
export function notificationsOf(sa: StressAgent): NotificationHelper {
	if (sa.notifications === null) {
		throw new Error(`${sa.name}'s notifications are not read by this run`);
	}
	return sa.notifications;
}

/** The lab networks: the ones a run joins, leaves and forgets. */
export function labNetworks(real: Real): WifiNetwork[] {
	return real.networks.filter(n => !n.home);
}

export function networkNamed(real: Real, name: string): WifiNetwork {
	const found = real.networks.find(n => n.ssid === name);
	if (found === undefined) throw new Error(`no network named ${name}`);
	return found;
}

export function hubNamed(real: Real, name: string): HubReal {
	const found = real.hubs.find(h => h.name === name);
	if (found === undefined) throw new Error(`no hub named ${name}`);
	return found;
}

/** Stop a hub's process, gracefully or not: the hub keeps its identity for
 * a later start. */
export async function parkHub(
	hub: HubReal,
	signal: 'SIGINT' | 'SIGKILL' = 'SIGINT',
): Promise<void> {
	if (hub.process === null) return;
	await stopLocalHub(hub.process, signal);
	hub.process = null;
}

/** Open `chat` from the home page, without checking it. Rows are matched on
 * their title element only — matching the whole row would collide with
 * message previews, which in groups quote sender names. The row appearing
 * (with its title, i.e. the peer profile synced) is itself a sync effect, so
 * it gets the cross-agent timeout. */
export async function openChatPage(
	sa: StressAgent,
	chat: ExpectedChat,
	model: ExpectedModel,
): Promise<ChatPage> {
	await backToChatList(sa, model);
	await clickChatRow(sa, model.chatListName(chat, sa.name));
	const page =
		chat.kind === 'direct' ? sa.agent.directChatPage : sa.agent.groupChatPage;
	await page.ready();
	model.openedChat(sa.name, chat);
	return page;
}

/** Open the chat the list shows under `title`, for reading a chat the model
 * does not know yet — which of the two pages it is is whatever appears. */
export async function openChatByTitle(
	sa: StressAgent,
	title: string,
): Promise<ChatPage> {
	await clickChatRow(sa, title);
	const { directChatPage, groupChatPage } = sa.agent;
	await sa.agent.waitUntil(
		async () =>
			(await directChatPage.page.isExisting()) ||
			(await groupChatPage.page.isExisting()),
		{ timeoutMsg: `${sa.name} opened "${title}" onto neither chat page` },
	);
	return (await directChatPage.page.isExisting())
		? directChatPage
		: groupChatPage;
}

/** Click the row the chat list shows under `title`. */
async function clickChatRow(sa: StressAgent, title: string): Promise<void> {
	try {
		await waitForChatRow(sa, title);
	} catch (err) {
		// A row that never took a new name and one whose operation never arrived
		// fail identically here, so say which it was.
		throw new Error(
			`${err instanceof Error ? err.message : String(err)}\n` +
				`${sa.name} derives its contact names from: ${JSON.stringify(
					await sa.agent.profileState(),
				)}`,
		);
	}
}

async function waitForChatRow(sa: StressAgent, title: string): Promise<void> {
	await sa.agent.waitUntil(
		() =>
			sa.agent.execute(
				(rowSel: string, title_: string) => {
					for (const row of document.querySelectorAll<HTMLElement>(rowSel)) {
						const rowTitle = row.querySelector<HTMLElement>(
							'.title-truncated-wrap > div:first-child',
						);
						if (rowTitle?.textContent?.includes(title_) === true) {
							(row.querySelector('a') ?? row).click();
							return true;
						}
					}
					return false;
				},
				tid('all-chats-row'),
				title,
			),
		{
			timeout: SYNC_TIMEOUT,
			timeoutMsg: `${sa.name} never saw a chat named "${title}" in its list`,
		},
	);
}

/** Get to the chat list, from a chat the agent still has open or from the
 * list it is already on. Moves leave the app wherever they finish, the way a
 * user does, so anything that needs the list starts by asking for it. */
export async function backToChatList(
	sa: StressAgent,
	model: ExpectedModel,
): Promise<void> {
	for (const page of [sa.agent.groupChatPage, sa.agent.directChatPage]) {
		if (!(await page.page.isExisting())) continue;
		await page.back.click();
		break;
	}
	await sa.agent.homePage.ready();
	model.wentHome(sa.name);
}

/** What entering a contact link gets before its chat has to be on screen.
 * Past the default because the publish it waits on queues behind whatever
 * the node is already doing for the peers it has, which on a phone adding
 * its n-th contact while several of them sync is seconds, not milliseconds.
 * The run logs how long each add really took. */
const ADD_CONTACT_TIMEOUT = 120_000;

/** Enter `peer`'s add-contact link on `sa` and record the add, which leaves
 * the app on their chat — where it also clears whatever `peer` had already
 * put on `sa`'s device, their contact request among it. */
export async function addContact(
	sa: StressAgent,
	peer: StressAgent,
	model: ExpectedModel,
): Promise<void> {
	if (peer.link === null) {
		throw new Error(`${peer.name}'s contact link was never collected`);
	}
	const started = Date.now();
	await backToChatList(sa, model);
	await navigateToAddContact(sa.agent);
	await sa.agent.addContactPage.enterAddContactLink(peer.link);
	await sa.agent.directChatPage.page.waitForExist({
		timeout: ADD_CONTACT_TIMEOUT,
		timeoutMsg: `${sa.name} never reached ${peer.name}'s chat after entering their link`,
	});
	log(`${sa.name}: added ${peer.name} in ${Date.now() - started}ms`);
	model.recordAdded(sa.name, peer.name);
	model.openedDirectChat(sa.name, peer.name);
}

/** Wait until the app is interactive again, wherever it came back: the chat
 * list, or the chat it was showing when it went away. */
export async function waitForApp(sa: StressAgent): Promise<void> {
	const { homePage, directChatPage, groupChatPage } = sa.agent;
	await sa.agent.waitUntil(
		async () =>
			(await homePage.settingsLink.isExisting()) ||
			(await directChatPage.page.isExisting()) ||
			(await groupChatPage.page.isExisting()),
		{ timeoutMsg: `${sa.name}'s app never came back` },
	);
}

/** Get back to the home page from wherever a failed or interrupted move
 * left the agent: a chat page, or home already. */
export async function ensureHome(
	sa: StressAgent,
	model: ExpectedModel,
): Promise<void> {
	model.wentHome(sa.name);
	for (const page of [sa.agent.groupChatPage, sa.agent.directChatPage]) {
		if (await page.page.isExisting()) {
			await page.back.click();
			await sa.agent.homePage.ready();
			return;
		}
	}
	if (await sa.agent.homePage.settingsLink.isExisting()) {
		await sa.agent.homePage.ready();
		return;
	}
	// A failed move can leave the agent on any page, with any dialog open —
	// nowhere a click path back home is known from, so the route is set
	// directly.
	await sa.agent.goto('/');
	await sa.agent.homePage.ready();
}
