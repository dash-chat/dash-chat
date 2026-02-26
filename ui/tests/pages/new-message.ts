import { S } from '../selectors';

export const selectors = S.newMessage;

/** Go back to the home page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Type into the search/filter bar */
export function search(query: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.search} input`,
		text: query,
	};
}

/** Assert the contact list is visible */
export function assertContactListVisible() {
	return `!!document.querySelector('${selectors.contactList}')`;
}
