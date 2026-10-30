import { describe, expect, it, vi } from 'vitest';

import { createBackTracker } from './navigation';

vi.mock('$app/navigation', () => ({
	afterNavigate: vi.fn(),
	goto: vi.fn(),
}));

/**
 * Browser history stand-in: a stack of URLs and a cursor, with the same
 * push/replace/back semantics the tracker is fed from `afterNavigate`.
 */
class FakeHistory {
	stack: string[];
	index = 0;

	constructor(initial: string) {
		this.stack = [initial];
	}

	get current(): string {
		return this.stack[this.index];
	}

	push(url: string) {
		this.stack = this.stack.slice(0, this.index + 1);
		this.stack.push(url);
		this.index += 1;
	}

	replace(url: string) {
		this.stack[this.index] = url;
	}

	back(): boolean {
		if (this.index === 0) return false;
		this.index -= 1;
		return true;
	}
}

/**
 * Drives a back tracker against a fake history exactly the way the app wires
 * it: each navigation feeds `handleNavigation` with its kind (browser backs
 * as popstate deltas), and `goBack` reproduces the real helper's decision.
 */
function createWalker(initial: string) {
	let tracker = createBackTracker();
	const history = new FakeHistory(initial);
	tracker.handleNavigation('enter');

	// The oracle the tracker is checked against: the pages the user walked
	// through since the app (re)started, in order. Back must unwind these.
	let journey: string[] = [initial];

	return {
		history,
		get current() {
			return history.current;
		},
		get journey() {
			return [...journey];
		},
		push(url: string) {
			history.push(url);
			tracker.handleNavigation('push');
			journey.push(url);
		},
		replace(url: string) {
			history.replace(url);
			tracker.handleNavigation('replace');
			journey[journey.length - 1] = url;
		},
		browserBack() {
			if (!history.back()) return;
			tracker.handleNavigation('popstate', -1);
			// Backing past the app-start point (possible after a reload) lands on
			// a page with no in-app history behind it, like a fresh start.
			if (journey.length > 1) journey.pop();
			else journey = [history.current];
		},
		goBack(fallback: string) {
			expect(tracker.canGoBack).toBe(journey.length > 1);
			if (tracker.canGoBack) {
				history.back();
				tracker.handleNavigation('popstate', -1);
				journey.pop();
			} else {
				this.replace(fallback);
			}
		},
		reload() {
			tracker = createBackTracker();
			tracker.handleNavigation('enter');
			journey = [history.current];
		},
	};
}

/** A representative navigation graph (the app's, at the time of writing):
 * pushes out of each page, and each page's Back fallback (its natural parent)
 * for when there is no history behind it. It only needs to be graph-shaped
 * like the app — the e2e back-buttons spec covers the real routes. */
const GRAPH: Record<string, { links: string[]; fallback: string }> = {
	'/': {
		links: ['/settings', '/new-message', '/direct-chats/c1', '/group-chat/g1'],
		fallback: '/',
	},
	'/settings': {
		links: [
			'/settings/profile',
			'/settings/account',
			'/settings/appearance',
			'/settings/notifications',
			'/settings/offline',
			'/settings/help',
			'/settings/profile/add-contact',
		],
		fallback: '/',
	},
	'/settings/profile': {
		links: [
			'/settings/profile/edit-name',
			'/settings/profile/edit-about',
			'/settings/profile/edit-photo',
			'/settings/profile/add-contact',
		],
		fallback: '/settings',
	},
	'/settings/profile/edit-name': { links: [], fallback: '/settings/profile' },
	'/settings/profile/edit-about': { links: [], fallback: '/settings/profile' },
	'/settings/profile/edit-photo': { links: [], fallback: '/settings/profile' },
	'/settings/profile/add-contact': {
		links: [],
		fallback: '/settings/profile',
	},
	'/settings/account': { links: [], fallback: '/settings' },
	'/settings/appearance': { links: [], fallback: '/settings' },
	'/settings/notifications': { links: [], fallback: '/settings' },
	'/settings/offline': { links: [], fallback: '/settings' },
	'/settings/help': {
		links: ['/settings/help/contact-us'],
		fallback: '/settings',
	},
	'/settings/help/contact-us': { links: [], fallback: '/settings/help' },
	'/new-message': {
		links: ['/new-message/add-contact', '/new-group', '/direct-chats/c1'],
		fallback: '/',
	},
	'/new-message/add-contact': { links: [], fallback: '/new-message' },
	'/new-group': { links: [], fallback: '/' },
	'/direct-chats/c1': {
		links: ['/direct-chats/c1/chat-settings'],
		fallback: '/',
	},
	'/direct-chats/c1/chat-settings': { links: [], fallback: '/direct-chats/c1' },
	'/group-chat/g1': { links: ['/group-chat/g1/info'], fallback: '/' },
	'/group-chat/g1/info': {
		links: ['/group-chat/g1/info/edit', '/group-chat/g1/info/add-members'],
		fallback: '/group-chat/g1',
	},
	'/group-chat/g1/info/edit': { links: [], fallback: '/group-chat/g1/info' },
	'/group-chat/g1/info/add-members': {
		links: [],
		fallback: '/group-chat/g1/info',
	},
};

// The test-banner feedback button is reachable from every page.
const FEEDBACK = '/settings/help/contact-us';

function mulberry32(seed: number) {
	let a = seed;
	return () => {
		a |= 0;
		a = (a + 0x6d2b79f5) | 0;
		let t = Math.imul(a ^ (a >>> 15), 1 | a);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

function pick<T>(rand: () => number, items: T[]): T {
	return items[Math.floor(rand() * items.length)];
}

describe('goBack', () => {
	it('random walks: Back always unwinds the steps taken, in order', () => {
		for (let seed = 1; seed <= 200; seed++) {
			const rand = mulberry32(seed);
			const walker = createWalker('/');

			for (let step = 0; step < 40; step++) {
				const node = GRAPH[walker.current];
				expect(
					node,
					`unknown page ${walker.current} (seed ${seed})`,
				).toBeDefined();
				const roll = rand();

				if (roll < 0.45 && node.links.length > 0) {
					walker.push(pick(rand, node.links));
				} else if (roll < 0.55 && walker.current !== FEEDBACK) {
					walker.push(FEEDBACK);
				} else if (roll < 0.8) {
					const journeyBefore = walker.journey;
					walker.goBack(node.fallback);
					const expected =
						journeyBefore.length > 1
							? journeyBefore[journeyBefore.length - 2]
							: node.fallback;
					expect(walker.current, `goBack (seed ${seed}, step ${step})`).toBe(
						expected,
					);
				} else if (roll < 0.9) {
					const journeyBefore = walker.journey;
					walker.browserBack();
					if (journeyBefore.length > 1) {
						expect(walker.current).toBe(
							journeyBefore[journeyBefore.length - 2],
						);
					}
				} else if (roll < 0.95) {
					walker.replace(pick(rand, [walker.current, node.fallback]));
				} else {
					walker.reload();
				}
			}

			// Fully unwind: Back must retrace the remaining journey in exact
			// reverse order, then fall back to the current page's parent.
			for (let guard = 0; walker.journey.length > 1 && guard < 100; guard++) {
				const expected = walker.journey[walker.journey.length - 2];
				walker.goBack(GRAPH[walker.current].fallback);
				expect(walker.current, `unwind (seed ${seed})`).toBe(expected);
			}
			const fallback = GRAPH[walker.current].fallback;
			walker.goBack(fallback);
			expect(walker.current).toBe(fallback);
		}
	});

	it('returns to the page feedback was opened from, wherever that was', () => {
		const walker = createWalker('/');
		walker.push('/direct-chats/c1');
		walker.push(FEEDBACK);
		walker.goBack(GRAPH[FEEDBACK].fallback);
		expect(walker.current).toBe('/direct-chats/c1');
	});

	it('falls back to the parent when the page starts the in-app history', () => {
		const walker = createWalker(FEEDBACK);
		walker.goBack(GRAPH[FEEDBACK].fallback);
		expect(walker.current).toBe('/settings/help');
	});

	it('does not count replace navigations as history entries', () => {
		const walker = createWalker('/direct-chats/c1');
		walker.replace('/direct-chats/c1');
		walker.goBack('/');
		expect(walker.current).toBe('/');
		expect(walker.history.stack).toEqual(['/']);
	});

	it('treats a reload as the start of the in-app history', () => {
		const walker = createWalker('/');
		walker.push('/settings');
		walker.push('/settings/help');
		walker.reload();
		walker.goBack(GRAPH['/settings/help'].fallback);
		expect(walker.current).toBe('/settings');
	});
});
