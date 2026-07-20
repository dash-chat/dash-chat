import { keyboard } from '$lib/utils/keyboard.svelte';

interface VirtualKeyboard {
	show(): void;
}

const MAX_HIDE_ATTEMPTS = 4;

// Identifies the latest hide request: bumping it cancels any pending retry,
// so a hide sequence can't outlive a reopenComposerKeyboard call and retract
// the keyboard it just brought back.
let hideToken = 0;

/** Deterministically retract the soft keyboard. Some Android IMEs (observed
 * on vivo) ignore every direct dismissal — blur(), VirtualKeyboard.hide(),
 * inputmode="none" on the focused element — but all of them retract when
 * focus moves to an input that requests no keyboard. A keyboard re-summoned
 * by `reopenComposerKeyboard` ignores even that (and blur/hide too) until
 * the IME is re-bound by focusing a regular text input, so the retraction
 * runs in two steps: bind, then move focus to a no-keyboard input. Even that
 * fails while the long-press touch is still down (the IME ignores bindings
 * made mid-touch), so the sequence retries until the viewport confirms the
 * keyboard actually closed. */
export function hideKeyboard() {
	attemptHide(++hideToken, 0);
}

function attemptHide(token: number, attempt: number) {
	if (token !== hideToken || attempt >= MAX_HIDE_ATTEMPTS) return;
	if (attempt > 0 && !keyboard.isOpen) return;
	const binder = makeDummyInput();
	binder.type = 'text';
	binder.focus({ preventScroll: true });
	setTimeout(() => {
		const dummy = makeDummyInput();
		dummy.inputMode = 'none';
		if (token === hideToken) dummy.focus({ preventScroll: true });
		setTimeout(() => {
			binder.remove();
			dummy.remove();
			attemptHide(token, attempt + 1);
		}, 250);
	}, 100);
}

/** An invisible input marked `data-keyboard-dummy` so focus handlers (e.g.
 * the composer's keyboard-slot logic) know its focus is IME plumbing, not a
 * user focusing an input. */
function makeDummyInput() {
	const input = document.createElement('input');
	input.dataset.keyboardDummy = '';
	input.style.position = 'fixed';
	input.style.opacity = '0';
	document.body.appendChild(input);
	return input;
}

/** Refocus the composer and re-show the soft keyboard (e.g. after the
 * message-actions overlay closes). Android WebView ignores programmatic focus
 * for IME purposes, and Chromium's VirtualKeyboard API honors show() only
 * under `virtualkeyboardpolicy="manual"` — so the policy is applied to the
 * textarea just for this call and removed right after, leaving the stock
 * automatic show/hide behavior in place. No-ops where the API is missing
 * (iOS, desktop WebKit). */
export function reopenComposerKeyboard() {
	hideToken++;
	const virtualKeyboard = (
		navigator as Navigator & { virtualKeyboard?: VirtualKeyboard }
	).virtualKeyboard;
	const textarea = document.querySelector<HTMLTextAreaElement>(
		'[data-testid="message-input-textarea"]',
	);
	if (!textarea) return;
	textarea.setAttribute('virtualkeyboardpolicy', 'manual');
	textarea.focus({ preventScroll: true });
	virtualKeyboard?.show();
	setTimeout(() => textarea.removeAttribute('virtualkeyboardpolicy'), 500);
}
