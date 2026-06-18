/** Resolve a CSS custom property (e.g. `--color-brand-primary`) to its computed value. */
export function cssColor(variable: string): string {
	if (typeof document === 'undefined') return '';
	return getComputedStyle(document.documentElement)
		.getPropertyValue(variable)
		.trim();
}
