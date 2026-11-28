import { defineConfig } from '@playwright/test';

const PREVIEW = 'http://localhost:4173';

// SCENES=a,b takes only those, to work on them without the rest.
const only = (process.env.SCENES ?? '')
	.split(',')
	.map(name => name.trim())
	.filter(name => name !== '');

export default defineConfig({
	testDir: '../../ui/src/routes',
	testMatch: '**/scenes.ts',
	outputDir: '../../.dbs/translation-screenshots-traces',
	fullyParallel: true,
	grep: only.length > 0 ? new RegExp(`(^| )(${only.join('|')})$`) : undefined,
	reporter: [['list'], ['./reporter.ts']],
	use: {
		baseURL: PREVIEW,
		// CHROME_BIN where the machine's Chrome is not where Playwright looks for it.
		...(process.env.CHROME_BIN === undefined
			? { channel: 'chrome' }
			: { launchOptions: { executablePath: process.env.CHROME_BIN } }),
		trace: 'retain-on-failure',
	},
	// The build `just i18n screenshots` leaves, of messages carrying their keys.
	webServer: {
		command: 'pnpm -F ui exec vite preview --port 4173 --strictPort',
		url: PREVIEW,
	},
});
