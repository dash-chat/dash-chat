import { S } from '../selectors';

export const selectors = S.newMessage;

/** Return the new-message page element if present */
export function newMessageLoaded() {
	return document.querySelector(selectors.back);
}

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

export function clickNewGroup(): void {
	const el = (document.querySelector(`${selectors.newGroup} a`) ??
		document.querySelector(selectors.newGroup)) as HTMLElement | null;
	if (!el) throw new Error('New group item not found');
	el.click();
}
