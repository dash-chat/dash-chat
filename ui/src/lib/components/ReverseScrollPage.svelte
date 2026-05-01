<!--
	A Konsta `<Page>` with a built-in column-reverse chat scroll container.

	Usage:
	  <ReverseScrollPage bind:el={scrollEl} data-testid="...">
	    <Navbar transparent />
	    ...messages and overlays...
	  </ReverseScrollPage>

	Children render inside the scroll wrapper. Place a Konsta `<Navbar>` first
	— its built-in `position: sticky; top: 0` keeps it pinned at the viewport
	top while content scrolls underneath in column-reverse order (latest
	message at the bottom, welcome card at the top).

	What it does:
	1. Suppresses scroll on `.k-page` (`overflow: hidden`) so the page itself
	   doesn't scroll. Reverted on unmount.
	2. Positions the scroll element as `absolute; inset: 0` inside `.k-page` so
	   the viewport extends from top to bottom.
	3. Makes the scroll element a column-reverse container (scrollTop=0 = bottom).
	4. An inner `flex: 1 0 auto` wrapper ensures children fill the viewport even
	   when content is short — without this the sticky navbar would sit at the
	   wrapper's natural top, which in column-reverse with under-viewport content
	   is somewhere in the middle of the screen.
	5. Manages the Material navbar bg opacity: opaque at the latest-message end,
	   transparent over the welcome card. iOS isn't touched — Konsta's gradient +
	   blur layers do the fading visually on their own.

	Why not Konsta's `scrollEl` prop: Konsta's progress formula clamps
	`scrollTop ≥ 0`, but WebKit reports negative scrollTop in column-reverse, so
	it would always compute progress=0 and never fade.

	Props mirror Konsta's `<Page>` (Konsta-specific options forwarded to the
	underlying Page). Plain HTML attributes (id, class, style, data-*, aria-*…)
	land on the inner scroll element — that's the element the consumer
	interacts with via `bind:el`.
-->
<script lang="ts">
	import { Page } from 'konsta/svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { SCROLL_BOTTOM_THRESHOLD } from '$lib/utils/chat';

	interface PageColors {
		bgIos?: string;
		bgMaterial?: string;
	}

	type ScrollToBottom = (animate?: boolean) => void;

	interface Props extends HTMLAttributes<HTMLDivElement> {
		component?: string;
		colors?: PageColors;
		ios?: boolean;
		material?: boolean;
		el?: HTMLDivElement | null;
		isAtBottom?: boolean;
		scrollToBottom?: ScrollToBottom;
		children?: Snippet;
	}

	let {
		component,
		colors,
		ios,
		material,
		el = $bindable(null),
		isAtBottom = $bindable(true),
		scrollToBottom = $bindable<ScrollToBottom>(() => {}),
		children,
		...scrollProps
	}: Props = $props();

	scrollToBottom = (animate = true) => {
		if (!el) return;
		el.scrollTo({ top: 0, behavior: animate ? 'smooth' : 'auto' });
	};

	const pageProps: Record<string, unknown> = $derived({
		component,
		colors,
		ios,
		material,
	});

	$effect(() => {
		const node = el;
		if (!node) return;

		node.style.position = 'absolute';
		node.style.inset = '0';
		node.style.overflowY = 'auto';
		node.style.display = 'flex';
		node.style.flexDirection = 'column-reverse';
		// Disable browser scroll anchoring — WebKit otherwise re-pins the scroll
		// position to the visual bottom whenever new content is appended in a
		// column-reverse container, even when the user has scrolled up to read
		// older messages.
		node.style.overflowAnchor = 'none';

		const pageEl = node.closest('.k-page') as HTMLElement | null;
		if (pageEl) {
			// Konsta's .k-page is `absolute overflow-auto` by default. We need
			// it as a positioning context for our absolute overlay, but we don't
			// want it to scroll itself.
			pageEl.style.overflow = 'hidden';
		}

		let navbarBgEl: HTMLElement | null = null;

		const updateNavbar = () => {
			// iOS theme: leave the navbar untouched. The gradient + blur fade
			// content into the background visually as it scrolls under them —
			// writing opacity here would defeat that.
			if (node.closest('.k-ios')) return;
			// Re-query if the cached element was removed (e.g. the Navbar got
			// swapped via {#if}{:else}, like toggling search mode in/out).
			if (!navbarBgEl || !navbarBgEl.isConnected) {
				// On Material there's a single `.k-navbar > div.absolute` (the bg).
				navbarBgEl =
					(node.querySelector(
						'.k-navbar > div.absolute',
					) as HTMLElement | null) ?? null;
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

		const updateIsAtBottom = () => {
			// WebKit uses negative scrollTop in column-reverse; abs() normalises.
			isAtBottom = Math.abs(node.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
		};

		const onScroll = () => {
			updateIsAtBottom();
			updateNavbar();
		};
		node.addEventListener('scroll', onScroll);

		updateIsAtBottom();

		// Watch the scroll subtree for child swaps (e.g. the Navbar being replaced
		// when search mode toggles) so we can re-resolve the navbar bg on the next
		// frame.
		const contentObserver = new MutationObserver(scheduleUpdate);
		contentObserver.observe(node, { childList: true, subtree: true });

		updateNavbar();

		return () => {
			if (frame) cancelAnimationFrame(frame);
			contentObserver.disconnect();
			node.removeEventListener('scroll', onScroll);
			if (pageEl) {
				pageEl.style.removeProperty('overflow');
			}
		};
	});
</script>

<Page {...pageProps}>
	<div bind:this={el} {...scrollProps}>
		<div style="flex: 1 0 auto;">
			{@render children?.()}
		</div>
	</div>
</Page>
