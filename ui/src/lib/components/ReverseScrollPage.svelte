<!--
	A Konsta `<Page>` with a built-in column-reverse chat scroll container.

	Usage:
	  <ReverseScrollPage data-testid="...">
	    {#snippet navbar()}
	      <Navbar transparent />
	    {/snippet}
	    ...messages and overlays...
	  </ReverseScrollPage>
-->
<script lang="ts" module>
	/** Distance from the bottom (in px) below which we consider the user
	 * "at the bottom" of the chat — controls when the scroll-to-bottom
	 * button hides and when self-sends snap back to the bottom. */
	export const SCROLL_BOTTOM_THRESHOLD = 200;
</script>

<script lang="ts">
	import { Page } from 'konsta/svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { findNavbarBg } from '$lib/utils/konsta';

	interface PageColors {
		bgIos?: string;
		bgMaterial?: string;
	}

	interface Props extends HTMLAttributes<HTMLDivElement> {
		component?: string;
		colors?: PageColors;
		ios?: boolean;
		material?: boolean;
		isAtBottom?: boolean;
		navbar?: Snippet;
		children?: Snippet;
	}

	let {
		component,
		colors,
		ios,
		material,
		isAtBottom = $bindable(true),
		navbar,
		children,
		...scrollProps
	}: Props = $props();

	let el: HTMLDivElement | null = $state(null);
	let innerEl: HTMLDivElement | null = $state(null);
	let suppressCompensateUntil = 0;
	let releaseSuppress: (() => void) | null = null;

	export function scrollToBottom(animate = true) {
		if (!el) return;
		const node = el;
		// Suppress scroll compensation for the duration of this scroll: the
		// caller wants the view to land at the bottom, not stay anchored to
		// older messages while the smooth animation runs. Release the gate as
		// soon as `scrollend` fires so a peer message arriving right after the
		// animation completes doesn't slide visibly within the 1s window. The
		// timeout is a fallback for cases where `scrollend` never fires (e.g.
		// already at the bottom so scrollTo is a no-op, or older WebKit).
		releaseSuppress?.();
		suppressCompensateUntil = performance.now() + 1000;

		let timeoutId: ReturnType<typeof setTimeout>;
		const release = () => {
			if (releaseSuppress !== release) return;
			releaseSuppress = null;
			clearTimeout(timeoutId);
			node.removeEventListener('scrollend', release);
			suppressCompensateUntil = 0;
		};
		releaseSuppress = release;
		node.addEventListener('scrollend', release, { once: true });
		timeoutId = setTimeout(release, 1000);

		node.scrollTo({ top: 0, behavior: animate ? 'smooth' : 'auto' });
	}

	const pageProps: Record<string, unknown> = $derived({
		component,
		colors,
		ios,
		material,
	});

	$effect(() => {
		const node = el;
		if (!node) return;
		const inner = innerEl;
		if (!inner) return;

		const pageEl = node.parentElement;
		if (!pageEl) return;

		let navbarBgEl: HTMLElement | null = null;
		let observedNavbar: HTMLElement | null = null;
		const navbarResizeObserver = new ResizeObserver(() => {
			if (observedNavbar) {
				node.style.setProperty(
					'--chat-navbar-height',
					`${observedNavbar.offsetHeight}px`,
				);
			}
		});

		const syncNavbar = () => {
			const navEl = pageEl.querySelector('.k-navbar') as HTMLElement | null;
			if (navEl !== observedNavbar) {
				if (observedNavbar) navbarResizeObserver.unobserve(observedNavbar);
				observedNavbar = navEl;
				if (navEl) {
					navbarResizeObserver.observe(navEl);
					node.style.setProperty(
						'--chat-navbar-height',
						`${navEl.offsetHeight}px`,
					);
				}
			}
			navbarBgEl = null;
		};

		const updateNavbar = () => {
			// iOS theme: leave the navbar untouched. The gradient + blur fade
			// content into the background visually as it scrolls under them —
			// writing opacity here would defeat that.
			if (node.closest('.k-ios')) return;
			// Re-query if the cached element was removed (e.g. the Navbar got
			// swapped via {#if}{:else}, like toggling search mode in/out).
			if (!navbarBgEl || !navbarBgEl.isConnected) {
				navbarBgEl = findNavbarBg(pageEl);
			}
			if (!navbarBgEl) return;

			// In column-reverse, scrollTop=0 is the visual bottom — the latest
			// message sits right under the navbar, so the bg should be opaque to
			// keep them visually separated. As the user scrolls up toward the top
			// of the content (welcome card / avatar area), the bg fades out so the
			// navbar blends with the welcome surface.
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			const maxScroll = node.scrollHeight - node.clientHeight;
			navbarBgEl.style.opacity =
				maxScroll < 1
					? '0'
					: maxScroll - Math.abs(node.scrollTop) > 10
						? '1'
						: '0';
		};

		// Coalesce mutation-driven updates to one per frame.
		let frame = 0;
		const scheduleUpdate = () => {
			if (frame) return;
			frame = requestAnimationFrame(() => {
				frame = 0;
				syncNavbar();
				updateNavbar();
			});
		};

		const updateIsAtBottom = () => {
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			isAtBottom = Math.abs(node.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
		};

		// Float shadow of node.scrollTop so successive compensations don't
		// accumulate the browser's per-set rounding error (WKWebView snaps
		// scrollTop to an integer; over N rapid incoming messages each loses
		// up to ~0.5px and the user sees a 1px-ish drift). We do the math in
		// floats and only the final write to node.scrollTop is rounded by the
		// browser; the next compensation builds on the precise float, not the
		// rounded readback.
		let desiredScrollTop = node.scrollTop;

		const onScroll = () => {
			// If the actual scrollTop diverged from our tracked float by more
			// than the rounding margin, treat it as a user scroll and re-sync.
			if (Math.abs(node.scrollTop - desiredScrollTop) > 1) {
				desiredScrollTop = node.scrollTop;
			}
			updateIsAtBottom();
			updateNavbar();
		};
		node.addEventListener('scroll', onScroll);

		updateIsAtBottom();

		// When the user is scrolled up reading older messages and a new message
		// arrives, the inner content grows at its DOM end (visual bottom in
		// column-reverse). The inner div's bottom is anchored to the container,
		// so its top extends upward — every existing message shifts up in
		// absolute terms while scrollTop stays fixed (overflow-anchor: none
		// disables compensation). The result: the messages the user was reading
		// scroll up out of view as newer content slides in from below.
		// Counter this by anchoring to the top of the content: when the inner
		// div grows while the user isn't at the bottom, shift scrollTop by the
		// growth amount so the same messages stay in view.
		//
		// Run the correction synchronously in the MutationObserver microtask
		// (not in a ResizeObserver) so it lands before the render phase. A
		// render-phase fix would mean the rAF scheduled here reads the new
		// scrollHeight with the old scrollTop and paints once with the
		// uncompensated layout — visible as a small scroll flicker before the
		// view resets. Forcing layout via inner.offsetHeight inside the MO
		// gives us the post-mutation height while the rAF and any paint are
		// still pending.
		// getBoundingClientRect().height returns a fractional value reflecting
		// the actual subpixel layout, while offsetHeight returns a rounded
		// integer. With offsetHeight, a real height delta of 30.4px reads as
		// 30, leaving a 0.4px residual visible as a one-pixel up/down jitter
		// after compensation; using the rect's float height keeps the
		// correction in lock-step with the layout.
		let prevInnerHeight = inner.getBoundingClientRect().height;
		const compensateScroll = () => {
			const newInnerHeight = inner.getBoundingClientRect().height;
			const delta = newInnerHeight - prevInnerHeight;
			prevInnerHeight = newInnerHeight;
			if (delta <= 0) return;
			if (performance.now() < suppressCompensateUntil) return;
			if (Math.abs(desiredScrollTop) < SCROLL_BOTTOM_THRESHOLD) return;
			// In a column-reverse container, WebKit reports scrollTop as
			// negative when the user has scrolled up from the visual bottom,
			// while Chromium reports it as positive. abs(scrollTop) is the
			// distance from the bottom in either engine. To keep the same
			// messages on screen as the inner div grows, we grow that
			// distance by `delta` in whichever sign convention applies — the
			// threshold check above guarantees scrollTop is non-zero here, so
			// Math.sign returns ±1.
			desiredScrollTop += Math.sign(desiredScrollTop) * delta;
			node.scrollTop = desiredScrollTop;
		};

		// Inner growth doesn't just need scroll compensation — when content
		// grows past the viewport for the first time, maxScroll jumps from 0
		// to >0 and the Material navbar bg should switch from transparent
		// (welcome card) to opaque. Without this, the navbar stays
		// transparent until the next user-driven scroll event re-runs
		// updateNavbar.
		const onInnerChange = () => {
			compensateScroll();
			updateNavbar();
		};

		// Run scroll compensation off mutations inside the scroll content only.
		// Scoping subtree:true to the inner div avoids forcing layout (via
		// getBoundingClientRect inside compensateScroll) on every unrelated
		// mutation under .k-page — search-input keystrokes, sheet/dialog
		// open-close, navbar text updates, etc.
		const innerObserver = new MutationObserver(onInnerChange);
		innerObserver.observe(inner, { childList: true, subtree: true });

		// Backup: a ResizeObserver on the inner div catches any size change the
		// MutationObserver missed (e.g. async-loaded web component content like
		// <wa-relative-time> resolving its rendered text after the initial
		// bubble paint). RO fires in the same render phase, before paint, so
		// it still pre-empts the visible flicker. If the MO already
		// compensated, prevInnerHeight is up to date and this RO call is a
		// no-op.
		const innerResizeObserver = new ResizeObserver(onInnerChange);
		innerResizeObserver.observe(inner);

		// Watch only the direct children of the page for navbar swaps (e.g.
		// when search mode toggles in/out the active <Navbar> element is
		// replaced). subtree:false means typing in the navbar's search input
		// or any other deep mutation doesn't wake this observer up.
		const pageObserver = new MutationObserver(scheduleUpdate);
		pageObserver.observe(pageEl, { childList: true, subtree: false });

		// Re-evaluate when the viewport shrinks/grows (e.g. the iOS keyboard
		// opening resizes the WKWebView frame) — clientHeight changes shift
		// maxScroll, so the opacity formula needs to re-run even when scrollTop
		// itself didn't move.
		const resizeObserver = new ResizeObserver(scheduleUpdate);
		resizeObserver.observe(node);

		syncNavbar();
		updateNavbar();

		return () => {
			if (frame) cancelAnimationFrame(frame);
			innerObserver.disconnect();
			pageObserver.disconnect();
			navbarResizeObserver.disconnect();
			resizeObserver.disconnect();
			innerResizeObserver.disconnect();
			node.removeEventListener('scroll', onScroll);
		};
	});
</script>

<Page {...pageProps} class="reverse-scroll-host">
	{@render navbar?.()}
	<!--
		overflow-anchor: none — disables browser scroll anchoring. WebKit
		otherwise re-pins the scroll position to the visual bottom whenever new
		content is appended in a column-reverse container, even when the user has
		scrolled up to read older messages.
	-->
	<div
		bind:this={el}
		class="absolute inset-0 flex flex-col-reverse overflow-y-auto"
		style="overflow-anchor: none"
		{...scrollProps}
	>
		<div
			bind:this={innerEl}
			style="flex: 1 0 auto; padding-top: var(--chat-navbar-height, 0px);"
		>
			{@render children?.()}
		</div>
	</div>
</Page>

<style>
	/*
		Konsta's `.k-page` is `absolute overflow-auto` by default. We use it
		as a positioning context for our absolute scroll element + sibling
		navbar, but we don't want it to scroll itself.
	*/
	:global(.reverse-scroll-host) {
		overflow: hidden;
	}
</style>
