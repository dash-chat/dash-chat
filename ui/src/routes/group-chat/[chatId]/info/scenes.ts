import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	GROUP_CHAT_PATH,
	ME,
	describeGroup,
} from '$lib/scenes/demo-world';
import type { Scene } from '$lib/scenes/scene';
import { MOCK_MAILBOX_DOWN, minutesAgo, mockBackend } from 'dash-chat-stores';

import { groupIAdminister } from '../scenes';

const info = `${GROUP_CHAT_PATH}/info`;

export default [
	{ name: 'group-info', path: info, seed: groupIAdminister },
	{
		name: 'group-member-actions',
		path: info,
		seed: groupIAdminister,
		steps: [{ tap: 'group-info-member-Alice' }],
	},
	{
		name: 'group-make-admin-dialog',
		path: info,
		seed: groupIAdminister,
		steps: [
			{ tap: 'group-info-member-Alice' },
			{ tap: 'group-info-make-admin' },
		],
	},
	{
		name: 'group-remove-member-dialog',
		path: info,
		seed: groupIAdminister,
		steps: [
			{ tap: 'group-info-member-Alice' },
			{ tap: 'group-info-remove-member' },
		],
	},
	{
		name: 'leave-group-dialog',
		path: info,
		seed: groupIAdminister,
		steps: [{ tap: 'group-info-leave' }],
	},
	{ name: 'group-edit', path: `${info}/edit`, seed: groupIAdminister },
	{
		name: 'group-add-members',
		path: `${info}/add-members`,
		seed: groupIAdminister,
	},
	{ name: 'home-with-group', path: '/', seed: groupIAdminister },
	{
		name: 'error-leaving-group-as-only-admin',
		path: info,
		seed() {
			groupIAdminister();
			mockBackend.failingCommands = ['leave_group:LastAdmin'];
		},
		steps: [{ tap: 'group-info-leave' }, { tap: 'group-info-leave-confirm' }],
	},
	{
		name: 'error-leaving-group',
		path: info,
		seed() {
			groupIAdminister();
			mockBackend.failingCommands = ['leave_group'];
		},
		steps: [{ tap: 'group-info-leave' }, { tap: 'group-info-leave-confirm' }],
	},
	{
		name: 'group-info-peers-online',
		path: info,
		seed() {
			groupIAdminister();
			mockBackend.mailboxes.cloudMailbox = MOCK_MAILBOX_DOWN;
			mockBackend.onlinePeers = [ALICE.agentId];
		},
	},
	{
		name: 'group-add-members-none-left',
		path: `${info}/add-members`,
		seed() {
			groupIAdminister();
			describeGroup(
				'Hiking club',
				[ME, ALICE, BOB, CAROL, DAVE],
				[ME],
				minutesAgo(1),
			);
		},
	},
] satisfies Scene[];
