import { scene } from '$lib/scenes';
import {
	ALICE,
	BOB,
	CAROL,
	DAVE,
	GROUP_CHAT_ID,
	GROUP_CHAT_PATH,
	ME,
	STRANGER,
	describeGroup,
	groupIAdminister,
	groupOperation,
	individual,
} from '$lib/scenes/demo-world';
import {
	MOCK_MAILBOX_DOWN,
	minutesAgo,
	mockBackend,
	setMockBackend,
	textMessage,
} from 'dash-chat-mock';

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
	textMessage(
		STRANGER.deviceId,
		GROUP_CHAT_ID,
		'Welcome everyone',
		minutesAgo(20),
	);
	describeGroup('Neighbours', [STRANGER, ME], [STRANGER], minutesAgo(59));
}

scene('group-chat-with-strangers', async ({ open }) => {
	groupAStrangerAdministers();
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-removed-by-a-stranger', async ({ open }) => {
	groupAStrangerAdministers();
	groupOperation(
		STRANGER,
		{ Remove: { member: individual(ME) } },
		minutesAgo(5),
	);
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-i-left', async ({ open }) => {
	groupAliceAdministers();
	groupOperation(ME, { Remove: { member: individual(ME) } }, minutesAgo(5));
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-peers-online', async ({ open }) => {
	groupIAdminister();
	setMockBackend({
		mailboxes: {
			...mockBackend().mailboxes,
			cloudMailbox: MOCK_MAILBOX_DOWN,
		},
		onlinePeers: [ALICE.agentId, DAVE.agentId],
	});
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-as-admin', async ({ open }) => {
	groupIAdminister();
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-as-member', async ({ open }) => {
	groupAliceAdministers();
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-removed', async ({ open }) => {
	groupAliceAdministers();
	groupOperation(ALICE, { Remove: { member: individual(ME) } }, minutesAgo(5));
	describeGroup('Book club', [ALICE], [ALICE], minutesAgo(59));
	await open(GROUP_CHAT_PATH);
});

scene('group-chat-nobody-known', async ({ open }) => {
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
});
