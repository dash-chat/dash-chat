import path from 'node:path';
import { pathToFileURL } from 'node:url';
import type { BrowserContextOptions } from 'playwright-core';

/** Paths are relative to the config file. */
export interface Config {
	/** Builds the app with every message carrying its key, run from the
	 * config's directory. */
	buildCommand: string;
	/** Where that leaves the app, a single-page one served as it is. */
	builtApp: string;
	/** Where the screenshots and the keys each one shows are saved. */
	screenshots: string;
	/** The JSON file of the source strings, by key. */
	sourceStrings: string;
	/** A JSON file listing, under the reason why, the keys of the source
	 * strings no scene can show. */
	stringsWithoutScreenshot: string;
	/** What a scene can be rendered as, by the name scenes call it. */
	devices: Record<string, BrowserContextOptions>;
	/** The device of a scene that names none. */
	defaultDevice: string;
	/** The key of the app's catch-all error message: a scene that shows it
	 * broke, unless its name starts with `error-`. */
	unexpectedErrorKey?: string;
	/** Where the screenshots are synced to. */
	weblate?: WeblateComponent;
}

export interface WeblateComponent {
	/** The address of the Weblate instance. */
	url: string;
	project: string;
	component: string;
}

export async function loadConfig(configFile: string): Promise<Config> {
	const absolute = path.resolve(configFile);
	const { default: config } = (await import(pathToFileURL(absolute).href)) as {
		default: Config;
	};
	const fromConfig = (relative: string) =>
		path.resolve(path.dirname(absolute), relative);
	return {
		...config,
		builtApp: fromConfig(config.builtApp),
		screenshots: fromConfig(config.screenshots),
		sourceStrings: fromConfig(config.sourceStrings),
		stringsWithoutScreenshot: fromConfig(config.stringsWithoutScreenshot),
	};
}
