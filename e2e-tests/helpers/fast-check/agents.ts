/**
 * The `Real` side of the model-based stress run — the driveable agents, plus
 * whatever networks and hubs a run creates — and the shared UI steps commands
 * are built from.
 */
import type { Hotspot } from '../../setup/hotspot';
import type { LocalHub } from '../../setup/local-hub';
import type { Agent } from '../../setup/setup-agents';
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
	/** This agent's add-contact link, once `prepareAgents` has collected it. */
	link: string | null;
}

/** A hub identity: its db, key and port survive its process, so bringing it
 * up on another network is the same hub moving, as a deployed one would. */
export interface HubReal {
	name: string;
	port: number;
	process: LocalHub | null;
}

/** fast-check's `Real`: the driveable agents, the Wi-Fi cards networks can be
 * raised on, and the networks and hubs the run has created so far. A spec
 * owns it so it can tear down whatever a run leaves behind. */
export interface Real {
	agents: StressAgent[];
	wifiDevices: string[];
	networks: Hotspot[];
	hubs: HubReal[];
}

/** A `Real` with nothing raised yet, able to put up one network per card in
 * `wifiDevices`. Pure: the agents are driven only by `prepareAgents`. */
export function newReal(init: {
	agents: { agent: Agent; name: string }[];
	wifiDevices?: string[];
}): Real {
	return {
		agents: init.agents.map(({ agent, name }) => ({ agent, name, link: null })),
		wifiDevices: init.wifiDevices ?? [],
		networks: [],
		hubs: [],
	};
}

export type ChatPage = DirectChatPage | GroupChatPage;

export function log(text: string): void {
	console.log(`[stress ${new Date().toISOString().slice(11, 23)}] ${text}`);
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

/** Open `chat` from the home page. Rows are matched on their title element
 * only — matching the whole row would collide with message previews, which
 * in groups quote sender names. The row appearing (with its title, i.e. the
 * peer profile synced) is itself a sync effect, so it gets the cross-agent
 * timeout. */
export async function openChat(
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
	if (peer.link === null) {
		throw new Error(`${peer.name}'s contact link was never collected`);
	}
	await navigateToAddContact(sa.agent);
	await sa.agent.addContactPage.enterAddContactLink(peer.link);
	await sa.agent.directChatPage.ready();
	await sa.agent.directChatPage.back.click();
	await sa.agent.homePage.ready();
}

/** Get back to the home page from wherever a failed or interrupted command
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
