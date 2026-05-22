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

export interface CheckOptions {
	checkDarkMode?: boolean;
	checkRTL?: boolean;
}

export async function checkOverflow(
	b: WebdriverIO.Browser,
): Promise<string[]> {
	return (await b.execute(() => {
		const isClipped = (el: Element) => {
			const s = window.getComputedStyle(el as HTMLElement);
			return (
				s.overflowX === 'hidden' ||
				s.overflowX === 'clip' ||
				s.overflow === 'hidden' ||
				s.overflow === 'clip' ||
				s.textOverflow === 'ellipsis'
			);
		};
		const issues: string[] = [];
		if (
			document.documentElement.scrollWidth >
			document.documentElement.clientWidth
		) {
			issues.push('Page has horizontal overflow');
		}
		document.querySelectorAll('*').forEach(el => {
			if (el.id === 'svelte-announcer') return;
			if (isClipped(el)) return;
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
	})) as string[];
}

export async function checkDarkMode(
	b: WebdriverIO.Browser,
): Promise<DarkModeResult> {
	return (await b.execute(() => {
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
	})) as DarkModeResult;
}

export async function checkRTL(b: WebdriverIO.Browser): Promise<RTLResult> {
	return (await b.execute(() => ({
		htmlDir: document.documentElement.dir,
		bodyDirection: getComputedStyle(document.body).direction,
	}))) as RTLResult;
}

export async function checkPage(
	b: WebdriverIO.Browser,
	options?: CheckOptions,
): Promise<CheckResult> {
	return (await b.execute((opts: CheckOptions | undefined) => {
		const isClipped = (el: Element) => {
			const s = window.getComputedStyle(el as HTMLElement);
			return (
				s.overflowX === 'hidden' ||
				s.overflowX === 'clip' ||
				s.overflow === 'hidden' ||
				s.overflow === 'clip' ||
				s.textOverflow === 'ellipsis'
			);
		};

		const overflow: string[] = [];
		if (
			document.documentElement.scrollWidth >
			document.documentElement.clientWidth
		) {
			overflow.push('Page has horizontal overflow');
		}
		document.querySelectorAll('*').forEach(el => {
			if (el.id === 'svelte-announcer') return;
			if (isClipped(el)) return;
			if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
				const text = el.textContent?.substring(0, 50);
				if (text?.trim()) {
					overflow.push(
						`Overflow in <${el.tagName.toLowerCase()}>: "${text.trim()}"`,
					);
				}
			}
		});

		const testIds = Array.from(document.querySelectorAll('[data-testid]'))
			.map(el => el.getAttribute('data-testid')!)
			.filter(Boolean);

		const navbar = document.querySelector('.k-navbar');
		const navbarText = navbar?.textContent?.trim() ?? '';

		const result: {
			testIds: string[];
			navbarText: string;
			overflow: string[];
			darkMode?: { isDark: boolean; issues: string[] };
			rtl?: { htmlDir: string; bodyDirection: string };
		} = { testIds, navbarText, overflow: overflow.slice(0, 20) };

		if (opts?.checkDarkMode) {
			const isDark = document.documentElement.classList.contains('dark');
			const darkIssues: string[] = [];
			if (!isDark) {
				darkIssues.push('Dark mode class not active');
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
						!htmlEl.closest(
							'wa-icon, wa-avatar, wa-qr-code, .qr-card, .k-toggle',
						)
					) {
						const tag = htmlEl.tagName.toLowerCase();
						const id =
							htmlEl.getAttribute('data-testid') ||
							htmlEl.className?.toString().substring(0, 40) ||
							'';
						darkIssues.push(`White bg in dark mode: <${tag}> ${id}`);
					}
				}
			});
			result.darkMode = { isDark, issues: darkIssues.slice(0, 20) };
		}

		if (opts?.checkRTL) {
			result.rtl = {
				htmlDir: document.documentElement.dir,
				bodyDirection: getComputedStyle(document.body).direction,
			};
		}

		return result;
	}, options)) as CheckResult;
}
