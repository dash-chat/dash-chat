import type { Scene } from '$lib/scenes/scene';

export default [
	{ name: 'new-message', path: '/new-message' },
	{
		name: 'new-message-no-match',
		path: '/new-message',
		steps: [{ type: 'zzzz', into: 'new-message-search' }],
	},
	{
		name: 'desktop-new-message-contact-menu',
		path: '/new-message',
		device: 'desktop',
		steps: [{ hover: 'new-message-contact-item' }],
	},
] satisfies Scene[];
