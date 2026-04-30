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
 *    (display:flex, flexDirection:column, overflow:hidden). The Page no
 *    longer scrolls — this action's element scrolls instead. Anyone
 *    reading the `<Page>` markup won't see this from the template; the
 *    side-effect lives here. Reverted on destroy().
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
	// Disable browser scroll anchoring — WebKit otherwise re-pins the scroll
	// position to the visual bottom whenever new content is appended in a
	// column-reverse container, even when the user has scrolled up to read
	// older messages.
	node.style.overflowAnchor = 'none';

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
			// TODO: tightly coupled to Konsta v5 internals — re-verify on upgrade.
			// In iOS theme there are two `.k-navbar > div.absolute` children (a
			// blur layer and the background); Konsta's bgElRef is the LAST one,
			// so pick that to stay in sync with Konsta's own opacity writes.
			const candidates = pageEl?.querySelectorAll('.k-navbar > div.absolute');
			navbarBgEl =
				(candidates?.[candidates.length - 1] as HTMLElement | undefined) ??
				null;
		}
		if (!navbarBgEl) return;

		// In column-reverse, scrollTop=0 is the visual bottom — the latest
		// message sits right under the navbar, so the bg should be opaque to
		// keep them visually separated. As the user scrolls up toward the top
		// of the content (welcome card / avatar area), the bg fades out so the
		// navbar blends with the welcome surface.
		// WebKit uses negative scrollTop in column-reverse; abs() normalises.
		const maxScroll = node.scrollHeight - node.clientHeight;
		if (maxScroll < 1) {
			navbarBgEl.style.opacity = '0';
		} else {
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

	// Keep the user's scroll position anchored to the visible bottom across
	// viewport resizes (e.g. keyboard show/hide). With column-reverse, the
	// browser already does this when scrollTop=0 (bottom-pinned), but in
	// WKWebView when the user is scrolled up, frame resize can leave the
	// content's bottom edge clipped behind the keyboard. We track the
	// last user-driven scrollTop and restore it on resize.
	let savedScrollTop = 0;
	let prevClientHeight = node.clientHeight;

	const onScroll = () => {
		// Ignore scroll events caused by a resize — clientHeight will differ
		// from the pre-resize value until the ResizeObserver catches up.
		if (node.clientHeight === prevClientHeight) {
			savedScrollTop = node.scrollTop;
		}
		updateNavbar();
	};
	node.addEventListener('scroll', onScroll);

	const sizeObserver = new ResizeObserver(() => {
		if (node.clientHeight !== prevClientHeight && savedScrollTop !== 0) {
			node.scrollTop = savedScrollTop;
		}
		prevClientHeight = node.clientHeight;
	});
	sizeObserver.observe(node);

	const contentObserver = new MutationObserver(scheduleUpdate);
	contentObserver.observe(node, { childList: true, subtree: true });

	// Watch the page for direct-child swaps (e.g. the Navbar element being
	// replaced when search mode toggles) so we can re-apply opacity to the
	// new navbar's bg. The .k-navbar is a direct child of .k-page, so
	// subtree: false is enough — avoids firing on every nested mutation.
	const pageObserver = pageEl ? new MutationObserver(scheduleUpdate) : null;
	pageObserver?.observe(pageEl!, { childList: true, subtree: false });

	updateNavbar();

	return {
		destroy() {
			if (frame) cancelAnimationFrame(frame);
			contentObserver.disconnect();
			sizeObserver.disconnect();
			pageObserver?.disconnect();
			node.removeEventListener('scroll', onScroll);
			if (pageEl) {
				pageEl.style.removeProperty('display');
				pageEl.style.removeProperty('flex-direction');
				pageEl.style.removeProperty('overflow');
			}
		},
	};
};
