import type { Scene } from '$lib/scenes/scene';

export default [
	{ name: 'help', path: '/settings/help' },
	{
		name: 'help-developer-mode-enabled',
		path: '/settings/help',
		steps: [
			() => {
				const version = document.querySelector<HTMLElement>(
					'[data-testid="help-version"]',
				);
				for (let tap = 0; tap < 7; tap++) version?.click();
			},
		],
	},
] satisfies Scene[];
