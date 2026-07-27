export interface DarkModeResult {
	isDark: boolean;
	issues: string[];
}

export interface RTLResult {
	htmlDir: string;
	bodyDirection: string;
	issues: string[];
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

export async function checkOverflow(b: WebdriverIO.Browser): Promise<string[]> {
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
		// Konsta mounts a popover, dialog or sheet — and its backdrop — up front
		// and keeps it at opacity 0 until it opens. They are wider than the
		// element that owns them (a message bubble owns both a reaction bar and
		// an actions menu), so they inflate its scrollWidth while painting
		// nothing at all.
		const isInvisible = (el: Element) => {
			for (let n: Element | null = el; n; n = n.parentElement) {
				if (window.getComputedStyle(n).opacity === '0') return true;
			}
			return false;
		};
		/** Descendants sticking out past `el`'s own box, i.e. what makes it
		 * overflow. Empty when the overflow comes from text rather than a child. */
		const culprits = (el: Element) => {
			const box = el.getBoundingClientRect();
			return Array.from(el.querySelectorAll('*')).filter(child => {
				const r = child.getBoundingClientRect();
				return (
					r.width > 0 && (r.right > box.right + 2 || r.left < box.left - 2)
				);
			});
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
				// Both walks are done only for an element that already overflows,
				// so the whole-document scan stays cheap.
				if (isInvisible(el)) return;
				const overflowing = culprits(el);
				if (overflowing.length > 0 && overflowing.every(isInvisible)) return;
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
	return (await b.execute(() => {
		const htmlDir = document.documentElement.dir;
		const bodyDirection = getComputedStyle(document.body).direction;
		const issues: string[] = [];
		if (htmlDir !== 'rtl')
			issues.push(`html dir is "${htmlDir}", expected "rtl"`);
		if (bodyDirection !== 'rtl')
			issues.push(`body direction is "${bodyDirection}", expected "rtl"`);
		return { htmlDir, bodyDirection, issues };
	})) as RTLResult;
}

export async function checkPage(
	b: WebdriverIO.Browser,
	options?: CheckOptions,
): Promise<CheckResult> {
	const [overflow, meta, darkMode, rtl] = await Promise.all([
		checkOverflow(b),
		b.execute(() => {
			const testIds = Array.from(document.querySelectorAll('[data-testid]'))
				.map(el => el.getAttribute('data-testid')!)
				.filter(Boolean);
			const navbar = document.querySelector('.k-navbar');
			const navbarText = navbar?.textContent?.trim() ?? '';
			return { testIds, navbarText };
		}) as Promise<{ testIds: string[]; navbarText: string }>,
		options?.checkDarkMode ? checkDarkMode(b) : Promise.resolve(undefined),
		options?.checkRTL ? checkRTL(b) : Promise.resolve(undefined),
	]);
	return { overflow, ...meta, darkMode, rtl };
}
