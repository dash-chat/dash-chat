import { showKeyboard } from 'tauri-plugin-virtual-keyboard';

let spacePreserved = $state(false);

export const keyboardSpace = {
	get preserved() {
		return spacePreserved;
	},
};

/** Hide the keyboard under an overlay without giving up its slot: the
 * composer opens its below-keyboard surface empty in the keyboard's place,
 * which retracts the keyboard natively while the input bar stays put. */
export function preserveKeyboardSpace() {
	spacePreserved = true;
}

export function releaseKeyboardSpace() {
	spacePreserved = false;
}

/** Refocus the composer and bring the keyboard back into the preserved slot
 * (e.g. after the message-actions overlay closes). Focus lands before the
 * surface closes, so the closing surface holds the inset until the rising
 * keyboard reclaims it and the input bar never dips. */
export function reopenComposerKeyboard() {
	const textarea = document.querySelector<HTMLTextAreaElement>(
		'[data-testid="message-input-textarea"]',
	);
	textarea?.focus({ preventScroll: true });
	releaseKeyboardSpace();
	void showKeyboard();
}
