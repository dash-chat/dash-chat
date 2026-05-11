// Type bridge for the e2e tests: re-augment `Window.__test` using the live
// surface declared in the UI package, and expose the locale-switch hook
// installed by the i18n bootstrap.

import type { testUtils } from '../ui/tests/setup-utils';

declare global {
	interface Window {
		__test: typeof testUtils;
		__setLocale: (locale: string) => Promise<void> | void;
	}

	namespace WebdriverIO {
		// These tests always run in multiremote mode (agent1 + agent2), so the
		// `browser` global is actually a MultiRemoteBrowser at runtime. Augment
		// the single-remote Browser type with the multiremote helpers we use.
		interface Browser {
			getInstance(browserName: string): WebdriverIO.Browser;
		}
	}
}

export {};
