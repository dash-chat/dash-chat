import { S } from '../selectors';

export const selectors = S.addContact;

/** Go back from the add-contact page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Switch to the code tab */
export function switchToCodeTab() {
	return { action: 'click' as const, selector: selectors.codeTab };
}

/** Switch to the scan tab */
export function switchToScanTab() {
	return { action: 'click' as const, selector: selectors.scanTab };
}

/** Copy the contact code */
export function copyCode() {
	return { action: 'click' as const, selector: selectors.copyButton };
}

/**
 * Paste a contact code into the input field.
 * ListInput puts data-testid on the outer <li>, so target the inner input.
 */
export function pasteCode(code: string) {
	return {
		action: 'type' as const,
		selector: `${selectors.codeInput} input`,
		text: code,
	};
}

/** Get the QR code value from the page */
export function getQrCodeValue() {
	return `document.querySelector('${selectors.qrCode}')?.getAttribute('value')`;
}
