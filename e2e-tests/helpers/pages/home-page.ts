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
		await this.chatListItem(contactName).waitForExist();
		await this.chatListItem(contactName).click();
		await this.agent.$(tid('direct-chat-messages')).waitForExist();
	}

	/** Click the chat-list row for `name` and resolve with the in-page ms from
	 * the click until `expectedMessages` bubbles are rendered in the group
	 * chat. The long timeout is deliberate: a slow chat should fail on the
	 * reported number, not on the poll. */
	async openGroupChatRenderMs(
		name: string,
		expectedMessages: number,
	): Promise<number> {
		await this.chatListItem(name).waitForExist();
		await this.agent.execute(
			(n: string, count: number) =>
				window.__test.measureGroupChatOpen(n, count),
			name,
			expectedMessages,
		);
		let ms: number | null = null;
		await this.agent.waitUntil(
			async () => {
				ms = await this.agent.execute(() => window.__test.groupChatOpenMs());
				return ms !== null;
			},
			{
				timeout: 300_000,
				timeoutMsg: `Group chat "${name}" never rendered ${expectedMessages} messages`,
			},
		);
		return ms!;
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
