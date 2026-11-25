import type { Scene } from '$lib/scenes/scene';
import { mockBackend } from 'dash-chat-stores';

export default [
	{ name: 'account', path: '/settings/account' },
	{
		name: 'delete-account-dialog',
		path: '/settings/account',
		steps: [{ tap: 'account-delete' }],
	},
	{
		name: 'error-deleting-account',
		path: '/settings/account',
		seed() {
			mockBackend.failingCommands = ['delete_account'];
		},
		steps: [{ tap: 'account-delete' }, { tap: 'account-delete-confirm' }],
	},
] satisfies Scene[];
