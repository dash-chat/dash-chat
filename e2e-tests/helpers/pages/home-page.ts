import { tid } from '../selectors';
import { TestHelper } from './test-helper';

const GET_STARTED_CARD_IDS = [
	'add-contact',
	'add-photo',
	'chat-color',
	'new-group',
] as const;

type GetStartedCardId = (typeof GET_STARTED_CARD_IDS)[number];

export class HomePage extends TestHelper {
	settingsLink = this.el(tid('home-settings-link'));
	newMessageButton = this.el(tid('home-new-message-btn'));
	firstChatTooltip = this.el(tid('first-chat-tooltip'));
	chatList = this.el(tid('all-chats-list'));
	chatRow = this.el(tid('all-chats-row'));
	emptyState = this.el(tid('all-chats-empty'));
	blockedRowIcon = this.el(tid('blocked-row-icon'));
	unreadBadge = this.el(tid('chat-row-unread-badge'));

	async ready() {
		await this.agent.waitUntil(() => this.isLoaded(), {
			timeout: 30_000,
			timeoutMsg: 'Home chat list (or empty state) did not render',
		});
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

	hasChatListItem(contactName: string) {
		return this.chatListItem(contactName).isExisting();
	}

	/** How many chats the list is showing. */
	async chatRowCount(): Promise<number> {
		return (await this.agent.$$(tid('all-chats-row'))).length;
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

	/** Open a chat by contact name and wait for the direct-chat page. Matched on
	 * the row's href, because a group row carries member names in its
	 * last-event summary and would otherwise win the text match. An arriving
	 * message re-renders the list, so a click can land on a row that is being
	 * replaced — retry until the chat is actually open. */
	async openChat(contactName: string): Promise<void> {
		await this.chatListItem(contactName).waitForExist();
		const messages = this.agent.$(tid('direct-chat-messages'));
		await this.agent.waitUntil(
			async () => {
				if (await messages.isExisting()) return true;
				const href = await this.directChatHref(contactName);
				if (href === null) return false;
				const row = this.agent.$(`${tid('all-chats-list')} a[href="${href}"]`);
				if (await row.isExisting()) await row.click();
				return messages.isExisting();
			},
			{ timeoutMsg: `Direct chat with "${contactName}" did not open` },
		);
	}

	/** The href of the chat-list row linking to the direct chat with
	 * `contactName`, or null while no such row is rendered. */
	private directChatHref(contactName: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, name: string) => {
				const rows = document.querySelectorAll<HTMLElement>(`${sel} a`);
				const row = Array.from(rows).find(
					r =>
						r.getAttribute('href')?.includes('/direct-chats/') === true &&
						(r.textContent ?? '').includes(name),
				);
				return row?.getAttribute('href') ?? null;
			},
			tid('all-chats-list'),
			contactName,
		);
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
