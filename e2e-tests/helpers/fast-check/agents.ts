/**
 * The `Real` side of the model-based stress run — the driveable agents — plus
 * the shared UI steps commands are built from.
 */
import type { Agent } from '../../setup/setup-agents';
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
	/** This agent's add-contact link, collected once at bootstrap. */
	link: string;
}

/** fast-check's `Real`: the driveable agents. */
export interface Real {
	agents: StressAgent[];
}

export type ChatPage = DirectChatPage | GroupChatPage;

export function log(text: string): void {
	console.log(`[stress] ${text}`);
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
