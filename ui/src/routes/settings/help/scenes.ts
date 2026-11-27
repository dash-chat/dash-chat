import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { find, tap } from 'i18n-screenshots/scene';

export default [
	{
		name: 'help',
		setup: () => open('/settings/help'),
	},
	{
		name: 'help-developer-mode-enabled',
		async setup() {
			await open('/settings/help');
			const version = await find('help-version');
			for (let tap = 0; tap < 7; tap++) version.click();
		},
	},
] satisfies Scene[];
