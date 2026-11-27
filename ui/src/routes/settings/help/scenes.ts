import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { tap } from 'translation-screenshots/scene';

export default [
	{
		name: 'help',
		setup: () => open('/settings/help'),
	},
	{
		name: 'help-developer-mode-enabled',
		async setup() {
			await open('/settings/help');
			const version = document.querySelector<HTMLElement>(
				'[data-testid="help-version"]',
			);
			for (let tap = 0; tap < 7; tap++) version?.click();
		},
	},
] satisfies Scene[];
