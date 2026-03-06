import { S } from '../selectors';
import { click } from '../helpers';

export const selectors = S.getStarted;

const cardIds = ['add-contact', 'add-photo', 'chat-color', 'new-group'] as const;

/** Return IDs of currently visible Get Started cards. */
export function visibleCards(): string[] {
	return cardIds.filter((id) => document.querySelector(`[data-testid="get-started-${id}"]`));
}

/** Dismiss a Get Started card by its id (e.g. 'add-contact'). */
export function dismissCard(cardId: string): void {
	click(S.getStarted.dismiss(cardId));
}
