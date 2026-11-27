import type { Scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	GROUP_CHAT_ID,
	GROUP_CHAT_PATH,
	ME,
	STRANGER,
	chatOperation,
	describeGroup,
	groupOperation,
	individual,
} from '$lib/scenes/demo-world';
import { open } from '$lib/scenes/steps';
import {
	MOCK_MAILBOX_DOWN,
	minutesAgo,
	mockBackend,
	setMockBackend,
} from 'dash-chat-mock';

export function groupIAdminister(): void {
	groupOperation(
		ME,
		{
			Create: {
				initial_members: [
					[individual(ME), null],
					[individual(ALICE), null],
					[individual(BOB), null],
				],
			},
		},
		minutesAgo(50),
	);
	groupOperation(
		ME,
		{ Add: { member: individual(CAROL), access: null } },
		minutesAgo(40),
	);
	groupOperation(
		ME,
		{ Promote: { member: individual(ALICE), access: null } },
		minutesAgo(30),
	);
	groupOperation(BOB, { Remove: { member: individual(BOB) } }, minutesAgo(20));
	groupOperation(
		ME,
		{ Demote: { member: individual(ALICE), access: null } },
		minutesAgo(15),
	);
	groupOperation(ME, { Remove: { member: individual(CAROL) } }, minutesAgo(10));
	describeGroup('Hiking club', [ME, ALICE, DAVE], [ME], minutesAgo(49));
}

function groupAliceAdministers(): void {
	groupOperation(
		ALICE,
		{
			Create: {
				initial_members: [
					[individual(ALICE), null],
					[individual(BOB), null],
				],
			},
		},
		minutesAgo(60),
	);
	groupOperation(
		ALICE,
		{ Add: { member: individual(ME), access: null } },
		minutesAgo(50),
	);
	groupOperation(
		ALICE,
		{ Add: { member: individual(CAROL), access: null } },
		minutesAgo(45),
	);
	groupOperation(
		ALICE,
		{ Promote: { member: individual(ME), access: null } },
		minutesAgo(40),
	);
	groupOperation(
		ALICE,
		{ Promote: { member: individual(BOB), access: null } },
		minutesAgo(35),
	);
	groupOperation(
		ALICE,
		{ Demote: { member: individual(BOB), access: null } },
		minutesAgo(30),
	);
	groupOperation(
		ALICE,
		{ Demote: { member: individual(ME), access: null } },
		minutesAgo(25),
	);
	groupOperation(
		ALICE,
		{ Remove: { member: individual(BOB) } },
		minutesAgo(20),
	);
	groupOperation(
		CAROL,
		{ Remove: { member: individual(CAROL) } },
		minutesAgo(15),
	);
	describeGroup('Book club', [ALICE, ME], [ALICE], minutesAgo(59));
}

function groupAStrangerAdministers(): void {
	groupOperation(
		STRANGER,
		{
			Create: {
				initial_members: [
					[individual(STRANGER), null],
					[individual(BOB), null],
				],
			},
		},
		minutesAgo(60),
	);
	groupOperation(
		STRANGER,
		{ Add: { member: individual(ME), access: null } },
		minutesAgo(50),
	);
	groupOperation(
		STRANGER,
		{ Promote: { member: individual(ME), access: null } },
		minutesAgo(40),
	);
	groupOperation(
		STRANGER,
		{ Demote: { member: individual(ME), access: null } },
		minutesAgo(35),
	);
	groupOperation(
		STRANGER,
		{ Remove: { member: individual(BOB) } },
		minutesAgo(30),
	);
	chatOperation(
		STRANGER.deviceId,
		GROUP_CHAT_ID,
		{ type: 'Message', payload: { message: 'Welcome everyone', media: null } },
		minutesAgo(20),
	);
	describeGroup('Neighbours', [STRANGER, ME], [STRANGER], minutesAgo(59));
}

export default [
	{
		name: 'group-chat-with-strangers',
		async setup() {
			groupAStrangerAdministers();
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-removed-by-a-stranger',
		async setup() {
			groupAStrangerAdministers();
			groupOperation(
				STRANGER,
				{ Remove: { member: individual(ME) } },
				minutesAgo(5),
			);
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-i-left',
		async setup() {
			groupAliceAdministers();
			groupOperation(ME, { Remove: { member: individual(ME) } }, minutesAgo(5));
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-peers-online',
		async setup() {
			groupIAdminister();
			setMockBackend({
				mailboxes: {
					...mockBackend.mailboxes,
					cloudMailbox: MOCK_MAILBOX_DOWN,
				},
				onlinePeers: [ALICE.agentId, DAVE.agentId],
			});
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-as-admin',
		async setup() {
			groupIAdminister();
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-as-member',
		async setup() {
			groupAliceAdministers();
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-removed',
		async setup() {
			groupAliceAdministers();
			groupOperation(
				ALICE,
				{ Remove: { member: individual(ME) } },
				minutesAgo(5),
			);
			describeGroup('Book club', [ALICE], [ALICE], minutesAgo(59));
			await open(GROUP_CHAT_PATH);
		},
	},
	{
		name: 'group-chat-nobody-known',
		async setup() {
			const otherStranger = {
				agentId: '0a'.repeat(32),
				deviceId: '1a'.repeat(32),
			};
			groupAStrangerAdministers();
			groupOperation(
				STRANGER,
				{ Remove: { member: individual(otherStranger) } },
				minutesAgo(10),
			);
			groupOperation(
				otherStranger,
				{ Remove: { member: individual(otherStranger) } },
				minutesAgo(8),
			);
			await open(GROUP_CHAT_PATH);
		},
	},
] satisfies Scene[];
