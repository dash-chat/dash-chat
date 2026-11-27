import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { fill, focus } from 'i18n-screenshots/scene';

export default [
	{
		name: 'new-message',
		setup: () => open('/new-message'),
	},
	{
		name: 'new-message-no-match',
		async setup() {
			await open('/new-message');
			await fill('new-message-search', 'zzzz');
		},
	},
	{
		name: 'desktop-new-message-contact-menu',
		device: 'desktop',
		async setup() {
			await open('/new-message');
			// Focus on a contact's menu button shows it as a pointer over the contact does.
			await focus('contact-menu-button');
		},
	},
] satisfies Scene[];
