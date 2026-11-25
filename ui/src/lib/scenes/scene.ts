import type { LocalStorageLogsClient } from 'dash-chat-stores';

/** Something done to the page once it has loaded, for what only shows after
 * an interaction: a dialog, a menu, a toast. A test id names what it is done
 * to; a function runs in the page, for what no gesture reaches. */
export type SceneStep =
	| { tap: string }
	| { hold: string }
	| { hover: string }
	| { type: string; into: string }
	| (() => void | Promise<void>);

/** One state of the UI worth showing a translator, described as data: the
 * preview's mock backend is seeded with it, and nothing has to be driven
 * there. */
export interface Scene {
	/** Names the screenshot. */
	name: string;
	path: string;
	/** What to render it as; an Android phone unless said otherwise. */
	device?: 'iphone' | 'desktop' | 'linux-desktop';
	/** Start from a device nobody has set up, not from the demo data. */
	firstLaunch?: boolean;
	/** Writes what this scene needs on top of that. */
	seed?: (logs: LocalStorageLogsClient) => void;
	steps?: SceneStep[];
	/** Keys of the strings it shows as pixels, which no marker can report:
	 * text the app draws into a picture. */
	drawn?: string[];
}
