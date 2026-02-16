import { S } from '../selectors';

export const selectors = S.createProfile;

/**
 * Type a name into the profile name field.
 * ListInput puts data-testid on the outer <li>, so target the inner input.
 */
export function typeName(name: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.nameInput} input`,
		text: name,
	};
}

/** Type a surname into the profile surname field */
export function typeSurname(surname: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.surnameInput} input`,
		text: surname,
	};
}

/** Click the create button (Material theme) */
export function clickCreate() {
	return { action: 'click' as const, selector: selectors.createButton };
}

/** Click the create link (iOS theme) */
export function clickCreateLink() {
	return { action: 'click' as const, selector: selectors.createLink };
}

/** Assert the create button is enabled */
export function assertCreateEnabled() {
	return `!document.querySelector('${selectors.createButton}')?.disabled`;
}
