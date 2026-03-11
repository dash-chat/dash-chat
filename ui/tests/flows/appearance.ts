/**
 * Appearance settings flows (language and theme selection).
 *
 * These run inside the browser context via window.__test.
 */

import { S } from '../selectors';
import { waitFor, click, nextTick } from '../helpers';

/**
 * Select a language via the appearance page dialog.
 *
 * Precondition: appearance page is loaded (appearance-language element visible).
 *
 * Opens the language dialog, clicks the radio input for the given locale,
 * and waits for the layout to re-render (the {#key localeKey} block in +layout.svelte
 * unmounts/remounts the entire app tree when locale changes).
 */
export async function selectLanguage(locale: string): Promise<true> {
	// Open the language dialog
	click(S.appearance.language);

	// Wait for the dialog option to appear
	const optionSelector = S.appearance.langOption(locale);
	await waitFor(optionSelector);
	await nextTick();

	// Click the radio input directly (bypasses pointer-events CSS on dialog wrapper)
	const input = document.querySelector(`${optionSelector} input[type="radio"]`) as HTMLInputElement;
	if (!input) throw new Error(`Radio input not found for locale ${locale}`);
	input.click();

	// Wait for the layout {#key} re-render to settle
	await new Promise((r) => setTimeout(r, 2000));

	return true;
}

/**
 * Select a theme via the appearance page.
 *
 * Handles both desktop (native <select>) and mobile (dialog) patterns.
 */
export async function selectTheme(scheme: 'system' | 'light' | 'dark'): Promise<true> {
	// Try native <select> first (desktop mode)
	const select = document.querySelector(`${S.appearance.theme} select`) as HTMLSelectElement | null;
	if (select) {
		select.value = scheme;
		select.dispatchEvent(new Event('change', { bubbles: true }));
	} else {
		// Mobile: open dialog and click option
		click(S.appearance.theme);
		await nextTick();

		const themeSelectors: Record<string, string> = {
			system: S.appearance.themeSystem,
			light: S.appearance.themeLight,
			dark: S.appearance.themeDark,
		};
		const optionSelector = themeSelectors[scheme];
		if (optionSelector && document.querySelector(optionSelector)) {
			(document.querySelector(optionSelector) as HTMLElement)?.click();
		}
	}

	await nextTick();
	return true;
}
