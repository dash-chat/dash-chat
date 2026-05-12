import { S } from '../selectors';

export const selectors = S.profile;

/** True if the profile-settings name list item contains `name`. */
export function profileNameListItemContains(name: string): boolean {
	const el = document.querySelector(selectors.editName);
	return !!el?.textContent?.includes(name);
}
