// Type bridge for the e2e tests:
// - Window.__test is declared by ui/tests/setup-utils.ts (pulled in via the
//   import below) — no need to redeclare it here.
// - `browser` and `expect` are runtime globals injected by WDIO; we declare
//   them ourselves so we can widen `browser` to MultiRemoteBrowser (these
//   tests are always multiremote) and avoid pulling in @wdio/globals.
import '../ui/tests/setup-utils';

declare global {
	// eslint-disable-next-line no-var
	var browser: WebdriverIO.MultiRemoteBrowser;
	// eslint-disable-next-line no-var
	var expect: ExpectWebdriverIO.Expect;
}
