import * as paraglide from '$lib/paraglide/runtime';

/**
 * Overwrite paraglide's `setLocale` so it never triggers a full page reload.
 * Cookie + global-variable strategies still get written (so `getLocale()`
 * returns the new value on the next call), and `onLocaleChange` fires with
 * the new locale so the caller can drive a re-render — typically a
 * `$state`-backed cache key in a `{#key}` block.
 *
 * Why no reload: paraglide's default behaviour orphans every in-flight Tauri
 * Channel callback when the page navigates away (`Channel::send` is
 * fire-and-forget; the JS handler disappears but the Rust forward task keeps
 * emitting), which floods the WebKitGTK event loop and stalls the next page
 * boot. Skipping the reload sidesteps that entirely.
 *
 * Safe to call more than once — the second call replaces the previous
 * callback without re-capturing the (already overwritten) `setLocale`.
 */
let originalSetLocale: typeof paraglide.setLocale | undefined;

export function registerSetLocale(
	onLocaleChange: (locale: paraglide.Locale) => void,
): void {
	if (!originalSetLocale) {
		// Capture the original before overwriting — `paraglide.setLocale` is a
		// live binding that follows `overwriteSetLocale`, so reading it later
		// would yield our overwritten function and recurse.
		originalSetLocale = paraglide.setLocale;
	}
	const original = originalSetLocale;
	paraglide.overwriteSetLocale(
		(newLocale: paraglide.Locale, options?: { reload?: boolean }) => {
			original(newLocale, { ...options, reload: false });
			onLocaleChange(newLocale);
		},
	);
}
