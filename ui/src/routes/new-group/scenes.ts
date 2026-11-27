import { onlyMyProfile } from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { tap } from 'translation-screenshots/scene';

export default [
	{
		name: 'new-group-members',
		setup: () => open('/new-group'),
	},
	{
		name: 'new-group-info',
		async setup() {
			await open('/new-group');
			await tap('selectable-contact-item');
			await tap('new-group-next');
		},
	},
	{
		name: 'ios-new-group-members',
		device: 'iphone',
		setup: () => open('/new-group'),
	},
	{
		name: 'new-group-no-contacts',
		async setup() {
			onlyMyProfile();
			await open('/new-group');
		},
	},
	{
		name: 'new-group-member-selected',
		async setup() {
			await open('/new-group');
			await tap('selectable-contact-item');
		},
	},
	{
		name: 'new-group-info-without-members',
		async setup() {
			await open('/new-group');
			await tap('new-group-next');
		},
	},
] satisfies Scene[];
