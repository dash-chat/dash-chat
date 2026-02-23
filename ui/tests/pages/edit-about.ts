import { S } from '../selectors';

export const selectors = S.editAbout;

/** Go back to the profile page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/**
 * Type into the about field.
 * ListInput puts data-testid on the outer <li>, so target the inner textarea.
 */
export function typeAbout(text: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.input} textarea`,
		text,
	};
}

/** Click save (Material theme) */
export function clickSave() {
	return { action: 'click' as const, selector: selectors.saveButton };
}

/** Click save (iOS theme) */
export function clickSaveLink() {
	return { action: 'click' as const, selector: selectors.saveLink };
}
