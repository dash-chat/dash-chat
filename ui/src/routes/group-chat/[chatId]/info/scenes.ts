import type { Scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	GROUP_CHAT_PATH,
	ME,
	describeGroup,
} from '$lib/scenes/demo-world';
import { open } from '$lib/scenes/steps';
import {
	MOCK_MAILBOX_DOWN,
	minutesAgo,
	mockBackend,
	setMockBackend,
} from 'dash-chat-mock';
import { tap } from 'i18n-screenshots/scene';

import { groupIAdminister } from '../scenes';

const info = `${GROUP_CHAT_PATH}/info`;

export default [
	{
		name: 'group-info',
		async setup() {
			groupIAdminister();
			await open(info);
		},
	},
	{
		name: 'group-member-actions',
		async setup() {
			groupIAdminister();
			await open(info);
			await tap('group-info-member-Alice');
		},
	},
	{
		name: 'group-make-admin-dialog',
		async setup() {
			groupIAdminister();
			await open(info);
			await tap('group-info-member-Alice');
			await tap('group-info-make-admin');
		},
	},
	{
		name: 'group-remove-member-dialog',
		async setup() {
			groupIAdminister();
			await open(info);
			await tap('group-info-member-Alice');
			await tap('group-info-remove-member');
		},
	},
	{
		name: 'leave-group-dialog',
		async setup() {
			groupIAdminister();
			await open(info);
			await tap('group-info-leave');
		},
	},
	{
		name: 'group-edit',
		async setup() {
			groupIAdminister();
			await open(`${info}/edit`);
		},
	},
	{
		name: 'group-add-members',
		async setup() {
			groupIAdminister();
			await open(`${info}/add-members`);
		},
	},
	{
		name: 'home-with-group',
		setup: groupIAdminister,
	},
	{
		name: 'error-leaving-group-as-only-admin',
		async setup() {
			groupIAdminister();
			setMockBackend({ failingCommands: ['leave_group:LastAdmin'] });
			await open(info);
			await tap('group-info-leave');
			await tap('group-info-leave-confirm');
		},
	},
	{
		name: 'error-leaving-group',
		async setup() {
			groupIAdminister();
			setMockBackend({ failingCommands: ['leave_group'] });
			await open(info);
			await tap('group-info-leave');
			await tap('group-info-leave-confirm');
		},
	},
	{
		name: 'group-info-peers-online',
		async setup() {
			groupIAdminister();
			setMockBackend({
				mailboxes: {
					...mockBackend.mailboxes,
					cloudMailbox: MOCK_MAILBOX_DOWN,
				},
				onlinePeers: [ALICE.agentId],
			});
			await open(info);
		},
	},
	{
		name: 'group-add-members-none-left',
		async setup() {
			groupIAdminister();
			describeGroup(
				'Hiking club',
				[ME, ALICE, BOB, CAROL, DAVE],
				[ME],
				minutesAgo(1),
			);
			await open(`${info}/add-members`);
		},
	},
] satisfies Scene[];
