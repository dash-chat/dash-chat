import type { Scene } from '$lib/scenes/scene';
import { open } from '$lib/scenes/steps';
import { setMockBackend } from 'dash-chat-mock';
import { tap } from 'i18n-screenshots/scene';

export default [
	{
		name: 'account',
		setup: () => open('/settings/account'),
	},
	{
		name: 'delete-account-dialog',
		async setup() {
			await open('/settings/account');
			await tap('account-delete');
		},
	},
	{
		name: 'error-deleting-account',
		async setup() {
			setMockBackend({ failingCommands: ['delete_account'] });
			await open('/settings/account');
			await tap('account-delete');
			await tap('account-delete-confirm');
		},
	},
] satisfies Scene[];
