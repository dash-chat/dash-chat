import { devices } from 'playwright-core';
import type { Config } from 'translation-screenshots/config';

const DESKTOP = {
	viewport: { width: 1100, height: 750 },
	deviceScaleFactor: 1.5,
};

export default {
	// `vite build` alone stops at the staging directory, so the build the app
	// embeds is left as it was.
	buildCommand: 'VITE_TRANSLATION_KEY_MARKERS=true pnpm exec vite build',
	builtApp: '.build-stage',
	screenshots: '../.dbs/translation-screenshots',
	sourceStrings: 'messages/en.json',
	stringsWithoutScreenshot: 'src/lib/scenes/strings-without-screenshot.json',
	unexpectedErrorKey: 'errorUnexpected',
	weblate: {
		url: 'https://nuance.weblate.cloud',
		project: 'dash-chat',
		component: 'dash-chat-app',
	},
	defaultDevice: 'phone',
	// Scaled so that no screenshot outgrows the 2000×2000 pixels Weblate accepts.
	devices: {
		phone: { ...devices['Pixel 7'], deviceScaleFactor: 2 },
		iphone: { ...devices['iPhone 14'], deviceScaleFactor: 2 },
		desktop: {
			...DESKTOP,
			userAgent:
				'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
		},
		'linux-desktop': {
			...DESKTOP,
			userAgent:
				'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
		},
	},
} satisfies Config;
