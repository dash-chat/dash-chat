import { tid } from '../../../ui/tests/selectors';
import { TestPage } from './test-page';

const GET_STARTED_CARD_IDS = [
	'add-contact',
	'add-photo',
	'chat-color',
	'new-group',
] as const;

type GetStartedCardId = (typeof GET_STARTED_CARD_IDS)[number];

export class HomePage extends TestPage {
	settingsLink = this.agent.$(tid('home-settings-link'));
	newMessageButton = this.agent.$(tid('home-new-message-btn'));
	firstChatTooltip = this.agent.$(tid('first-chat-tooltip'));
	chatList = this.agent.$(tid('all-chats-list'));
	chatRow = this.agent.$(tid('all-chats-row'));
	emptyState = this.agent.$(tid('all-chats-empty'));

	async ready() {
		await Promise.race([
			this.chatList.waitForExist(),
			this.emptyState.waitForExist(),
		]);
	}

	/** Chat-list entry whose link text contains `contactName`. */
	getChatListItem(contactName: string) {
		return this.chatList.$(`a*=${contactName}`);
	}

	hasChatListItem(contactName: string) {
		return this.getChatListItem(contactName).isExisting();
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
						issues.push(
							`Overflow in <${el.tagName.toLowerCase()}>: "${text}"`,
						);
				}
			});
			return issues.slice(0, 10);
		}, tid('all-chats-list'));
	}
}
