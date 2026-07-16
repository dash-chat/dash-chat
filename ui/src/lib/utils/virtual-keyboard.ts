interface VirtualKeyboard {
	show(): void;
}

/** Deterministically retract the soft keyboard. Some Android WebViews
 * (observed on vivo) ignore every direct dismissal — blur(),
 * VirtualKeyboard.hide(), inputmode="none" on the focused element — but all
 * of them retract when focus moves to an input that requests no keyboard. */
export function hideKeyboard() {
	const dummy = document.createElement('input');
	dummy.inputMode = 'none';
	dummy.style.position = 'fixed';
	dummy.style.opacity = '0';
	document.body.appendChild(dummy);
	dummy.focus({ preventScroll: true });
	setTimeout(() => dummy.remove(), 400);
}

/** Refocus the composer and re-show the soft keyboard (e.g. after the
 * message-actions overlay closes). Android WebView ignores programmatic focus
 * for IME purposes, and Chromium's VirtualKeyboard API honors show() only
 * under `virtualkeyboardpolicy="manual"` — so the policy is applied to the
 * textarea just for this call and removed right after, leaving the stock
 * automatic show/hide behavior in place. No-ops where the API is missing
 * (iOS, desktop WebKit). */
export function reopenComposerKeyboard() {
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
