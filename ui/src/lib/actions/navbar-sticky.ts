import type { Action } from 'svelte/action';

/**
 * Sticks an element below the page's Konsta `<Navbar>`. Each scope (the
 * nearest `.k-page` ancestor) shares a single `ResizeObserver` (on the
 * navbar) and `MutationObserver` (on the page subtree, to catch navbar
 * swaps when search mode toggles) — so N consumers cost 1 set of observers,
 * not N. Subscribers receive the navbar's current height and write their
 * own `top`.
 *
 * Usage:
 *   <div use:navbarSticky>...</div>           // 8px gap (default)
 *   <div use:navbarSticky={12}>...</div>      // custom gap below navbar
 */

type Subscriber = (height: number) => void;

interface Tracker {
	subscribe(s: Subscriber): () => void;
}

const trackers = new WeakMap<Element, Tracker>();

function getOrCreate(scope: Element): Tracker {
	const existing = trackers.get(scope);
	if (existing) return existing;

	const subscribers = new Set<Subscriber>();
	let currentNavbar: HTMLElement | null = null;
	let currentHeight = 0;

	const notify = () => {
		for (const s of subscribers) s(currentHeight);
	};

	const ro = new ResizeObserver(entries => {
		const entry = entries[0];
		if (!entry) return;
		currentHeight = entry.borderBoxSize[0].blockSize;
		notify();
	});

	const sync = () => {
		const navbar = scope.querySelector('.k-navbar') as HTMLElement | null;
		if (navbar === currentNavbar) return;
		if (currentNavbar) ro.unobserve(currentNavbar);
		currentNavbar = navbar;
		if (navbar) {
			ro.observe(navbar);
			currentHeight = navbar.offsetHeight;
			notify();
		}
	};

	// Coalesce subtree mutations to one rAF — the page subtree fires for
	// every new message, reaction toggle, etc., but the navbar identity
	// changes very rarely (search mode toggle).
	let frame = 0;
	const scheduleSync = () => {
		if (frame) return;
		frame = requestAnimationFrame(() => {
			frame = 0;
			sync();
		});
	};

	const mo = new MutationObserver(scheduleSync);
	mo.observe(scope, { childList: true, subtree: true });
	sync();

	const tracker: Tracker = {
		subscribe(s) {
			subscribers.add(s);
			if (currentNavbar) s(currentHeight);
			return () => {
				subscribers.delete(s);
				if (subscribers.size === 0) {
					if (frame) cancelAnimationFrame(frame);
					ro.disconnect();
					mo.disconnect();
					trackers.delete(scope);
				}
			};
		},
	};

	trackers.set(scope, tracker);
	return tracker;
}

export const navbarSticky: Action<HTMLElement, number | undefined> = (
	node,
	gap = 8,
) => {
	const scope = node.closest('.k-page');
	if (!scope) return;
	node.style.position = 'sticky';
	const unsubscribe = getOrCreate(scope).subscribe(height => {
		node.style.top = `${height + gap}px`;
	});
	return {
		destroy() {
			unsubscribe();
			node.style.removeProperty('position');
			node.style.removeProperty('top');
		},
	};
};
