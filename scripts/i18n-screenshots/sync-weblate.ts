import { syncScreenshotsToWeblate } from 'i18n-screenshots-weblate';

import { readScreenshotsOfEveryScene } from './files.ts';
import { renderedKey } from './rendered-keys.ts';

const token = process.env.WEBLATE_TOKEN;
if (!token) throw new Error('WEBLATE_TOKEN is not set');

await syncScreenshotsToWeblate({
	screenshots: readScreenshotsOfEveryScene(),
	url: 'https://nuance.weblate.cloud',
	token,
	project: 'dash-chat',
	component: 'dash-chat-app',
	sourceLanguage: 'en',
	renderedKey,
	log: console.log,
});
