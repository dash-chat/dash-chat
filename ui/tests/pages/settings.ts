import { S } from '../selectors';

export const selectors = S.settings;

/** Go back to the home page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Navigate to profile settings */
export function goToProfile() {
	return { action: 'click' as const, selector: selectors.profileLink };
}

/** Navigate to QR code / add contact */
export function goToQr() {
	return { action: 'click' as const, selector: selectors.qrLink };
}

/** Navigate to account settings */
export function goToAccount() {
	return { action: 'click' as const, selector: selectors.accountLink };
}
