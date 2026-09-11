/**
 * The `Real` side of a fuzz run — the driveable agents, plus whatever
 * networks and hubs a run creates — and the UI navigation moves are built
 * from. Nothing here compares a screen with the model: that is `checks.ts`.
 */
import { type LocalHub, stopLocalHub } from '../../setup/local-hub';
import type { Agent } from '../../setup/setup-agents';
import type { WifiNetwork } from '../../setup/test-env';
import { navigateToAddContact } from '../flows/exchange-contacts';
import type { DirectChatPage } from '../pages/direct-chats/direct-chat-page';
import type { GroupChatPage } from '../pages/group-chat/group-chat-page';
import { tid } from '../selectors';
import { SYNC_TIMEOUT } from '../timeouts';
import type { ExpectedChat, ExpectedModel } from './model';

// Mirrors QUICK_EMOJIS in ui/src/lib/utils/emojis.ts.
export const QUICK_EMOJIS = ['❤️', '👍', '👎', '😂', '😮', '😢'];

export interface StressAgent {
	agent: Agent;
	/** The profile first name, as chat lists show it. Must be unique across
	 * the run's agents and not a substring of another agent's name. */
	name: string;
	/** This agent's add-contact link, once preparation has collected it. */
	link: string | null;
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
	 * own home network last when its card is on one. */
	networks: WifiNetwork[];
	/** The host's Wi-Fi card, null when it has none or no network is
	 * configured. */
	hubsDevice: string | null;
	/** The SSID the card is on, null while off. Where every hub is today: a
	 * later slice with a card (or a machine) per hub replaces this with a
	 * location on `HubReal`. */
	hubsNetwork: string | null;
	hubs: HubReal[];
}

/** A `Real` with no hub yet and the card off every test network. Pure: the
 * agents are driven only by preparation. */
export function newReal(init: {
	agents: { agent: Agent; name: string }[];
	networks?: WifiNetwork[];
	hubsDevice?: string | null;
}): Real {
	const networks = init.networks ?? [];
	return {
		agents: init.agents.map(({ agent, name }) => ({ agent, name, link: null })),
		networks,
		hubsDevice: networks.length === 0 ? null : (init.hubsDevice ?? null),
		hubsNetwork: null,
		hubs: [],
	};
}

export type ChatPage = DirectChatPage | GroupChatPage;

export function log(text: string): void {
	console.log(`[fuzz ${new Date().toISOString().slice(11, 23)}] ${text}`);
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
	await sa.agent.homePage.ready();
	const name = model.chatListName(chat, sa.name);
	await sa.agent.waitUntil(
		() =>
			sa.agent.execute(
				(rowSel: string, name_: string) => {
					for (const row of document.querySelectorAll<HTMLElement>(rowSel)) {
						const title = row.querySelector<HTMLElement>(
							'.title-truncated-wrap > div:first-child',
						);
						if (title?.textContent?.includes(name_) === true) {
							(row.querySelector('a') ?? row).click();
							return true;
						}
					}
					return false;
				},
				tid('all-chats-row'),
				name,
			),
		{
			timeout: SYNC_TIMEOUT,
			timeoutMsg: `${sa.name} never saw a chat named "${name}" in its list`,
		},
	);
	const page =
		chat.kind === 'direct' ? sa.agent.directChatPage : sa.agent.groupChatPage;
	await page.ready();
	return page;
}

export async function goHome(sa: StressAgent, page: ChatPage): Promise<void> {
	await page.back.click();
	await sa.agent.homePage.ready();
}

/** Enter `peer`'s add-contact link on `sa`, from the home page and back to it. */
export async function addContact(
	sa: StressAgent,
	peer: StressAgent,
): Promise<void> {
	if (peer.link === null) {
		throw new Error(`${peer.name}'s contact link was never collected`);
	}
	await navigateToAddContact(sa.agent);
	await sa.agent.addContactPage.enterAddContactLink(peer.link);
	await sa.agent.directChatPage.ready();
	await sa.agent.directChatPage.back.click();
	await sa.agent.homePage.ready();
}

/** Get back to the home page from wherever a failed or interrupted move
 * left the agent: a chat page, or home already. */
export async function ensureHome(sa: StressAgent): Promise<void> {
	for (const page of [sa.agent.groupChatPage, sa.agent.directChatPage]) {
		if (await page.page.isExisting()) {
			await goHome(sa, page);
			return;
		}
	}
	await sa.agent.homePage.ready();
}
