import { onlyMyProfile } from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';

export default [
	{ name: 'new-group-members', path: '/new-group' },
	{
		name: 'new-group-info',
		path: '/new-group',
		steps: [{ tap: 'selectable-contact-item' }, { tap: 'new-group-next' }],
	},
	{ name: 'ios-new-group-members', path: '/new-group', device: 'iphone' },
	{
		name: 'new-group-no-contacts',
		path: '/new-group',
		firstLaunch: true,
		seed: onlyMyProfile,
	},
	{
		name: 'new-group-member-selected',
		path: '/new-group',
		steps: [{ tap: 'selectable-contact-item' }],
	},
	{
		name: 'new-group-info-without-members',
		path: '/new-group',
		steps: [{ tap: 'new-group-next' }],
	},
] satisfies Scene[];
