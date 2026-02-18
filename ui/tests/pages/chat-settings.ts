import { S } from '../selectors';

export const selectors = S.chatSettings;

/** Go back to the chat */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Get the peer name displayed in settings */
export function getPeerName() {
	return `document.querySelector('${selectors.peerName}')?.textContent`;
}

/** Click the search button */
export function clickSearch() {
	return { action: 'click' as const, selector: selectors.searchButton };
}
