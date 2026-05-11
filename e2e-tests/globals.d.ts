// Type bridge for the e2e tests: re-augment `Window.__test` using the live
// surface declared in the UI package, expose the locale-switch hook installed
// by the i18n bootstrap, and declare the runtime globals provided by WDIO.

import type { testUtils } from '../ui/tests/setup-utils';

declare global {
	interface Window {
		__test: typeof testUtils;
		__setLocale: (locale: string) => Promise<void> | void;
	}

	// These tests always run in multiremote mode (agent1 + agent2), so the
	// `browser` global is actually a MultiRemoteBrowser at runtime. Declare it
	// as such here instead of poisoning WebdriverIO.Browser with multiremote
	// methods, which would falsely advertise them on any Browser value flowing
	// through the type system.
	// eslint-disable-next-line no-var
	var browser: WebdriverIO.MultiRemoteBrowser;
	// eslint-disable-next-line no-var
	var expect: ExpectWebdriverIO.Expect;
}

export {};
