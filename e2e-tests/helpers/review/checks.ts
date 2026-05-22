/**
 * Automated check functions for review testing.
 * Called at each page during visitAllPages() or standalone via window.__test.
 */

export interface DarkModeResult {
	isDark: boolean;
	issues: string[];
}

export interface RTLResult {
	htmlDir: string;
	bodyDirection: string;
}

export interface CheckResult {
	testIds: string[];
	navbarText: string;
	overflow: string[];
	darkMode?: DarkModeResult;
	rtl?: RTLResult;
}

export interface PageResult extends CheckResult {
	page: string;
}

export function isIntentionallyClipped(el: Element): boolean {
	const style = window.getComputedStyle(el as HTMLElement);
	return (
		style.overflowX === 'hidden' ||
		style.overflowX === 'clip' ||
		style.overflow === 'hidden' ||
		style.overflow === 'clip' ||
		style.textOverflow === 'ellipsis'
	);
}

/** Scan all elements for horizontal overflow. */
export function checkOverflow(): string[] {
	const issues: string[] = [];
	if (
		document.documentElement.scrollWidth > document.documentElement.clientWidth
	) {
		issues.push('Page has horizontal overflow');
	}
	document.querySelectorAll('*').forEach(el => {
		if (el.id === 'svelte-announcer') return;
		if (isIntentionallyClipped(el)) return;
		if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
			const text = el.textContent?.substring(0, 50);
			if (text?.trim()) {
				issues.push(
					`Overflow in <${el.tagName.toLowerCase()}>: "${text.trim()}"`,
				);
			}
		}
	});
	return issues.slice(0, 20);
}

/** Check dark mode state and scan for hardcoded white backgrounds. */
export function checkDarkMode(): DarkModeResult {
	const isDark = document.documentElement.classList.contains('dark');
	const issues: string[] = [];
	if (!isDark) {
		issues.push('Dark mode class not active');
	}
	document.querySelectorAll('*').forEach(el => {
		const htmlEl = el as HTMLElement;
		const bg = getComputedStyle(htmlEl).backgroundColor;
		if (
			bg === 'rgb(255, 255, 255)' &&
			htmlEl.offsetWidth > 0 &&
			htmlEl.offsetHeight > 0
		) {
			if (
				!htmlEl.closest('wa-icon, wa-avatar, wa-qr-code, .qr-card, .k-toggle')
			) {
				const tag = htmlEl.tagName.toLowerCase();
				const id =
					htmlEl.getAttribute('data-testid') ||
					htmlEl.className?.toString().substring(0, 40) ||
					'';
				issues.push(`White bg in dark mode: <${tag}> ${id}`);
			}
		}
	});
	return { isDark, issues: issues.slice(0, 20) };
}

/** Check RTL direction state. */
export function checkRTL(): RTLResult {
	return {
		htmlDir: document.documentElement.dir,
		bodyDirection: getComputedStyle(document.body).direction,
	};
}

/** Run all applicable checks for the current page. */
export function checkPage(options?: {
	checkDarkMode?: boolean;
	checkRTL?: boolean;
}): CheckResult {
	const testIds = Array.from(document.querySelectorAll('[data-testid]'))
		.map(el => el.getAttribute('data-testid')!)
		.filter(Boolean);

	const navbar = document.querySelector('.k-navbar');
	const navbarText = navbar?.textContent?.trim() ?? '';

	const overflow = checkOverflow();

	const result: CheckResult = { testIds, navbarText, overflow };

	if (options?.checkDarkMode) {
		result.darkMode = checkDarkMode();
	}
	if (options?.checkRTL) {
		result.rtl = checkRTL();
	}

	return result;
}
