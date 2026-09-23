import { tid } from '../selectors';
import { TestHelper } from './test-helper';

/** Spelt once: `tid()` yields CSS, and the row lookup below needs XPath. */
const CHAT_LIST_ID = 'all-chats-list';

const GET_STARTED_CARD_IDS = [
	'add-contact',
	'add-photo',
	'chat-color',
	'new-group',
] as const;

type GetStartedCardId = (typeof GET_STARTED_CARD_IDS)[number];

/** One row of the chat list: what it is titled and what its unread badge
 *  reads. */
export interface ChatRow {
	title: string;
	unread: number;
}

/** How many times [`HomePage.openChat`] clicks the row before giving up. Each
 *  attempt costs a full `waitforTimeout`, so this stays small. */
const OPEN_CHAT_ATTEMPTS = 2;

/** `text` as an XPath string literal. XPath 1.0 has no escape, so a value
 *  containing a quote has to be assembled with `concat`. */
function xpathLiteral(text: string): string {
	if (!text.includes('"')) return `"${text}"`;
	const parts = text.split('"').map(part => `"${part}"`);
	return `concat(${parts.join(", '\"', ")})`;
}

export class HomePage extends TestHelper {
	settingsLink = this.el(tid('home-settings-link'));
	newMessageButton = this.el(tid('home-new-message-btn'));
	firstChatTooltip = this.el(tid('first-chat-tooltip'));
	chatList = this.el(tid(CHAT_LIST_ID));
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

	/** Chat-list entry whose link text contains `contactName`. Queried from the
	 * document rather than off the list element: the list is absent until the
	 * store hydrates, and a child query on a missing parent throws where every
	 * caller here is waiting for the row to turn up. */
	chatListItem(contactName: string) {
		return this.agent.$(
			`//*[@data-testid="${CHAT_LIST_ID}"]//a[contains(., ${xpathLiteral(contactName)})]`,
		);
	}

	hasChatListItem(contactName: string) {
		return this.chatListItem(contactName).isExisting();
	}

	/** How many chats the list is showing. */
	async chatRowCount(): Promise<number> {
		return (await this.agent.$$(tid('all-chats-row'))).length;
	}

	/** The title of every chat in the list: a peer's name for a direct chat,
	 * the group's name for a group. Read from the title element alone, since a
	 * row's summary quotes message text and sender names. */
	async chatTitles(): Promise<string[]> {
		return (await this.chatRows()).map(row => row.title);
	}

	/** Every chat in the list by its title, with the number its unread badge
	 * reads — 0 for a row showing none. */
	async chatRows(): Promise<ChatRow[]> {
		return this.agent.execute(
			(rowSel: string, badgeSel: string) => {
				const rows = document.querySelectorAll<HTMLElement>(rowSel);
				return Array.from(rows).map(row => ({
					title: (
						row.querySelector<HTMLElement>(
							'.title-truncated-wrap > div:first-child',
						)?.textContent ?? ''
					).trim(),
					unread: Number(
						row.querySelector<HTMLElement>(badgeSel)?.textContent?.trim() ??
							'0',
					),
				}));
			},
			tid('all-chats-row'),
			tid('chat-row-unread-badge'),
		);
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
	 * replaced — retry until the chat is actually open.
	 *
	 * Each attempt waits for the chat with its own `waitForExist` rather than
	 * polling inside one `waitUntil`: on Android the first element query after
	 * the navigation blocks until the webview answers again, which took the
	 * whole shared budget and left the loop with a single attempt — reported as
	 * "did not open" against a chat that was open and rendered. */
	async openChat(contactName: string): Promise<void> {
		await this.chatListItem(contactName).waitForExist();
		const messages = this.agent.$(tid('direct-chat-messages'));
		for (let attempt = 1; ; attempt++) {
			if (await messages.isExisting()) return;
			const href = await this.directChatHref(contactName);
			if (href !== null) {
				const row = this.agent.$(`${tid('all-chats-list')} a[href="${href}"]`);
				if (await row.isExisting()) await row.click();
			}
			try {
				await messages.waitForExist();
				return;
			} catch {
				if (attempt === OPEN_CHAT_ATTEMPTS) {
					throw new Error(`Direct chat with "${contactName}" did not open`);
				}
			}
		}
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
