import { S } from '../selectors';

export const selectors = S.newGroup;

/** Go back from the members page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Click next (Material theme) */
export function clickNext() {
	return { action: 'click' as const, selector: selectors.nextButton };
}

/** Click next (iOS theme) */
export function clickNextLink() {
	return { action: 'click' as const, selector: selectors.nextLink };
}

/** Go back from the group info page */
export function goBackFromInfo() {
	return { action: 'click' as const, selector: selectors.infoBack };
}

/**
 * Type the group name.
 * ListInput puts data-testid on the outer <li>, so target the inner input.
 */
export function typeGroupName(name: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.nameInput} input`,
		text: name,
	};
}

/** Click create (Material theme) */
export function clickCreate() {
	return { action: 'click' as const, selector: selectors.createButton };
}

/** Click create (iOS theme) */
export function clickCreateLink() {
	return { action: 'click' as const, selector: selectors.createLink };
}
