import { tid } from '../selectors';
import { TestPage } from './test-page';

const GET_STARTED_CARD_IDS = [
	'add-contact',
	'add-photo',
	'chat-color',
	'new-group',
] as const;

type GetStartedCardId = (typeof GET_STARTED_CARD_IDS)[number];

// The chat list lives inside an `{#await}` block that re-renders whenever the
// summaries re-resolve (e.g. while stores hydrate on a cold start with
// existing chats). A remount invalidates previously resolved element handles,
// and wry reports that as a generic error WebdriverIO won't auto-refetch from
// — so these accessors are getters that build a fresh element chain on every
// access instead of caching one handle for the page object's lifetime.
export class HomePage extends TestPage {
	get settingsLink() {
		return this.agent.$(tid('home-settings-link'));
	}
	get newMessageButton() {
		return this.agent.$(tid('home-new-message-btn'));
	}
	get firstChatTooltip() {
		return this.agent.$(tid('first-chat-tooltip'));
	}
	get chatList() {
		return this.agent.$(tid('all-chats-list'));
	}
	get chatRow() {
		return this.agent.$(tid('all-chats-row'));
	}
	get emptyState() {
		return this.agent.$(tid('all-chats-empty'));
	}

	async ready() {
		await this.agent.waitUntil(
			async () => {
				try {
					return await this.isLoaded();
				} catch {
					return false;
				}
			},
			{ timeoutMsg: 'Home page (chat list or empty state) did not appear' },
		);
	}

	async isLoaded(): Promise<boolean> {
		return (
			(await this.chatList.isExisting()) || (await this.emptyState.isExisting())
		);
	}

	/** Chat-list entry whose link text contains `contactName`. */
	chatListItem(contactName: string) {
		return this.chatList.$(`a*=${contactName}`);
	}

	async hasChatListItem(contactName: string): Promise<boolean> {
		try {
			return await this.chatListItem(contactName).isExisting();
		} catch {
			return false;
		}
	}

	/** Full visible text of the first chat-list row containing `name`. */
	async chatRowText(name: string): Promise<string> {
		await this.chatListItem(name).waitForExist();
		return this.agent.execute(
			(sel: string, nameArg: string) => {
				const rows = Array.from(document.querySelectorAll<HTMLElement>(sel));
				const row = rows.find(r => r.innerText.includes(nameArg));
				return row?.innerText ?? '';
			},
			tid('all-chats-row'),
			name,
		);
	}

	/** Open a chat by contact name and wait for the direct-chat page. */
	async openChat(contactName: string): Promise<void> {
		await this.agent.waitUntil(() => this.hasChatListItem(contactName), {
			timeoutMsg: `Chat list item "${contactName}" did not appear`,
		});
		try {
			await this.chatListItem(contactName).click();
		} catch {
			// The list can remount between the existence check and the click
			// (summaries re-resolving); one fresh retry is enough.
			await this.chatListItem(contactName).click();
		}
		await this.agent.$(tid('direct-chat-messages')).waitForExist();
	}

	getStartedCard(id: GetStartedCardId) {
		return this.agent.$(tid(`get-started-${id}`));
	}

	dismissGetStartedCardButton(id: GetStartedCardId) {
		return this.agent.$(tid(`get-started-dismiss-${id}`));
	}

	async visibleGetStartedCards(): Promise<GetStartedCardId[]> {
		const checks = await Promise.all(
			GET_STARTED_CARD_IDS.map(id => this.getStartedCard(id).isExisting()),
		);
		return GET_STARTED_CARD_IDS.filter((_, i) => checks[i]);
	}

	/** Returns descriptions of any chat-list items overflowing their container. */
	checkChatListOverflow(): Promise<string[]> {
		return this.agent.execute((selector: string) => {
			const issues: string[] = [];
			const list = document.querySelector(selector);
			if (!list) {
				issues.push('Chat list not found');
				return issues;
			}
			if (list.scrollWidth > list.clientWidth + 2) {
				issues.push('Chat list container has horizontal overflow');
			}
			list.querySelectorAll<HTMLElement>('*').forEach(el => {
				const style = window.getComputedStyle(el);
				const clipped =
					style.overflowX === 'hidden' ||
					style.overflowX === 'clip' ||
					style.overflow === 'hidden' ||
					style.overflow === 'clip' ||
					style.textOverflow === 'ellipsis';
				if (clipped) return;
				if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
					const text = el.textContent?.substring(0, 60).trim();
					if (text)
						issues.push(`Overflow in <${el.tagName.toLowerCase()}>: "${text}"`);
				}
			});
			return issues.slice(0, 10);
		}, tid('all-chats-list'));
	}
}
