import { S } from '../selectors';

export const selectors = S.newGroup;

export function newGroupLoaded(): Element | null {
	return document.querySelector(selectors.back);
}

/** Go back from the members page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

export function clickNewGroupNext(): void {
	const el = document.querySelector(selectors.next) as HTMLElement | null;
	if (!el) throw new Error('New group next button not found');
	el.click();
}

export function clickNewGroupCreate(): void {
	const el = document.querySelector(selectors.create) as HTMLElement | null;
	if (!el) throw new Error('New group create button not found');
	el.click();
}

/** Go back from the group info page */
export function goBackFromInfo() {
	return { action: 'click' as const, selector: selectors.infoBack };
}
