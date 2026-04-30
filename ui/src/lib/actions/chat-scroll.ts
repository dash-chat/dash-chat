import type { Action } from 'svelte/action';

/**
 * Svelte action that turns an element into a column-reverse chat scroll
 * container and manages the parent Konsta Page's transparent navbar opacity.
 *
 * Apply to a div inside a `<Page>` with a `<Navbar transparent>`:
 *
 *   <Page>
 *     <Navbar transparent />
 *     <div use:chatScroll>
 *       <div style="flex: 1 0 auto">  <!-- stretches content upward when short -->
 *         ...messages...
 *       </div>
 *     </div>
 *   </Page>
 *
 * What it does:
 * 1. Overrides the parent .k-page to be a non-scrolling flex column
 * 2. Makes this element a column-reverse scroll container (scrollTop=0 = bottom)
 * 3. Manages the transparent navbar's background opacity based on scroll position
 */
export const chatScroll: Action<HTMLElement> = node => {
	// Style this element as a column-reverse scroll container.
	node.style.flex = '1';
	node.style.minHeight = '0';
	node.style.overflowY = 'auto';
	node.style.display = 'flex';
	node.style.flexDirection = 'column-reverse';

	// Override the parent Konsta Page to be a non-scrolling flex column.
	const pageEl = node.closest('.k-page') as HTMLElement | null;
	if (pageEl) {
		pageEl.style.display = 'flex';
		pageEl.style.flexDirection = 'column';
		pageEl.style.overflow = 'hidden';
	}

	// Navbar opacity management — Konsta's transparent navbar tracks
	// scroll events from the Page element. Since the Page has overflow:hidden,
	// Konsta always sees scrollTop=0 and keeps the bg invisible. We override
	// the inline opacity directly.
	let navbarBgEl: HTMLElement | null = null;

	const updateNavbar = () => {
		// Re-query if the cached element was removed (e.g. the Navbar got
		// swapped via {#if}{:else}, like toggling search mode in/out).
		if (!navbarBgEl || !navbarBgEl.isConnected) {
			navbarBgEl = pageEl?.querySelector('.k-navbar > div.absolute') ?? null;
		}
		if (!navbarBgEl) return;

		const maxScroll = node.scrollHeight - node.clientHeight;
		if (maxScroll < 1) {
			navbarBgEl.style.opacity = '0';
		} else {
			// In column-reverse, scrollTop=0 is the visual bottom.
			// WebKit uses negative scrollTop; abs() normalises.
			const distFromTop = maxScroll - Math.abs(node.scrollTop);
			navbarBgEl.style.opacity = distFromTop > 10 ? '1' : '0';
		}
	};

	// Coalesce mutation-driven updates to one per frame — the subtree
	// observer fires on every DOM change (new messages, reactions, etc.),
	// and we only need the latest layout state per paint.
	let frame = 0;
	const scheduleUpdate = () => {
		if (frame) return;
		frame = requestAnimationFrame(() => {
			frame = 0;
			updateNavbar();
		});
	};

	node.addEventListener('scroll', updateNavbar);

	const contentObserver = new MutationObserver(scheduleUpdate);
	contentObserver.observe(node, { childList: true, subtree: true });

	// Watch the page for direct-child swaps (e.g. the Navbar element being
	// replaced when search mode toggles) so we can re-apply opacity to the
	// new navbar's bg.
	const pageObserver = pageEl ? new MutationObserver(scheduleUpdate) : null;
	pageObserver?.observe(pageEl!, { childList: true });

	updateNavbar();

	return {
		destroy() {
			if (frame) cancelAnimationFrame(frame);
			contentObserver.disconnect();
			pageObserver?.disconnect();
			node.removeEventListener('scroll', updateNavbar);
			if (pageEl) {
				pageEl.style.display = '';
				pageEl.style.flexDirection = '';
				pageEl.style.overflow = '';
			}
		},
	};
};
