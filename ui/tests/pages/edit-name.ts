import { S } from '../selectors';

export const selectors = S.editName;

/** Go back to the profile page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/**
 * Type a name.
 * ListInput puts data-testid on the outer <li>, so target the inner input.
 */
export function typeName(name: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.nameInput} input`,
		text: name,
	};
}

/**
 * Type a surname.
 * ListInput puts data-testid on the outer <li>, so target the inner input.
 */
export function typeSurname(surname: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.surnameInput} input`,
		text: surname,
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
