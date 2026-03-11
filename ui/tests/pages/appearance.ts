import { S } from '../selectors';

export const selectors = S.appearance;

/** Navigate to appearance settings */
export function goToAppearance() {
	return { action: 'click' as const, selector: S.settings.appearanceLink };
}
