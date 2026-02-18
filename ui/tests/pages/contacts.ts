import { S } from '../selectors';

export const selectors = S.contacts;

/** Go back to the home page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Navigate to add contact */
export function goToAddContact() {
	return { action: 'click' as const, selector: selectors.addLink };
}

/** Assert the contacts list is visible */
export function assertListVisible() {
	return `!!document.querySelector('${selectors.list}')`;
}
