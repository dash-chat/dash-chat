import {
	type AgentId,
	type ChatId,
	type ChatPayload,
	type DeviceGroupPayload,
	type DeviceId,
	type Hash,
	type Payload,
	type Profile,
	personalTopicFor,
} from 'dash-chat-stores';

import { LocalStorageLogsClient, chatIdForDevices } from './client';

export const DEMO_IDS = {
	MY_AGENT_ID: 'aa'.repeat(32),
	MY_DEVICE_ID: 'bb'.repeat(32),
	DEVICE_GROUP_TOPIC: 'cc'.repeat(32),
	INBOX_TOPIC: 'dd'.repeat(32),
};

export const DEMO_CONTACTS = [
	{
		agentId: '01'.repeat(32),
		deviceId: '11'.repeat(32),
		name: 'Alice',
		surname: 'Johnson',
		about: 'Signal enthusiast',
	},
	{
		agentId: '02'.repeat(32),
		deviceId: '12'.repeat(32),
		name: 'Bob',
		surname: 'Smith',
		about: 'Privacy advocate',
	},
	{
		agentId: '03'.repeat(32),
		deviceId: '13'.repeat(32),
		name: 'Carol',
		surname: undefined,
		about: 'Encrypted by default',
	},
	{
		agentId: '04'.repeat(32),
		deviceId: '14'.repeat(32),
		name: 'Dave',
		surname: 'Wilson',
		about: undefined,
	},
];

export const DEMO_CONTACT_REQUESTER = {
	agentId: '05'.repeat(32),
	deviceId: '15'.repeat(32),
	name: 'Eve',
	surname: 'Taylor',
};

/** The device pubkey of each demo identity, for agent → device lookups. */
export const DEMO_CONTACT_DEVICES: Record<string, string> = Object.fromEntries(
	[...DEMO_CONTACTS, DEMO_CONTACT_REQUESTER].map(contact => [
		contact.agentId,
		contact.deviceId,
	]),
);

export function minutesAgo(minutes: number): number {
	return Date.now() - minutes * 60_000;
}

interface Person {
	agentId: AgentId;
	deviceId: DeviceId;
}

/** Write a chat operation as `author`'s device, resolving to its hash. */
export function chatOperation(
	author: DeviceId,
	chatId: ChatId,
	payload: ChatPayload,
	timestamp: number,
): Hash {
	const operation: Payload = { type: 'Chat', payload };
	return new LocalStorageLogsClient(author).createSync(
		chatId,
		operation,
		timestamp,
	);
}

/** Write a message of nothing but text as `author`'s device, resolving to
 * its hash. */
export function textMessage(
	author: DeviceId,
	chatId: ChatId,
	message: string,
	timestamp: number,
): Hash {
	return chatOperation(
		author,
		chatId,
		{ type: 'Message', payload: { message, media: null } },
		timestamp,
	);
}

/** Write an operation of mine to my device group: who I added, blocked… */
export function deviceGroupOperation(
	payload: DeviceGroupPayload,
	timestamp: number,
): Hash {
	const operation: Payload = { type: 'DeviceGroupPayload', payload };
	return new LocalStorageLogsClient(DEMO_IDS.MY_DEVICE_ID).createSync(
		DEMO_IDS.DEVICE_GROUP_TOPIC,
		operation,
		timestamp,
	);
}

/** Announce `profile` as `person`'s. */
export function profileOperation(
	person: Person,
	profile: Profile,
	timestamp?: number,
): Hash {
	const operation: Payload = {
		type: 'Announcements',
		payload: { type: 'SetProfile', payload: profile },
	};
	return new LocalStorageLogsClient(person.deviceId).createSync(
		personalTopicFor(person.agentId),
		operation,
		timestamp,
	);
}

export function seedDemoData() {
	if (localStorage.getItem('__preview_seeded')) return;

	const me = DEMO_IDS.MY_DEVICE_ID;
	profileOperation(
		{ agentId: DEMO_IDS.MY_AGENT_ID, deviceId: me },
		{
			name: 'You',
			surname: undefined,
			avatar: undefined,
			about: 'Testing preview mode',
		},
		minutesAgo(120),
	);
	for (const contact of DEMO_CONTACTS) {
		profileOperation(
			contact,
			{
				name: contact.name,
				surname: contact.surname,
				avatar: undefined,
				about: contact.about,
			},
			minutesAgo(100),
		);
	}
	DEMO_CONTACTS.forEach((contact, i) => {
		deviceGroupOperation(
			{
				type: 'AddContact',
				payload: {
					agent_id: contact.agentId,
					direct_chat_topic_id: chatIdForDevices(me, contact.deviceId),
				},
			},
			minutesAgo(90 - i),
		);
	});

	// Dave is a contact with no messages.
	const [alice, bob, carol] = DEMO_CONTACTS.map(contact => contact.deviceId);

	const aliceChat = chatIdForDevices(me, alice);
	textMessage(
		alice,
		aliceChat,
		'Hey! Have you tried the new encrypted messaging?',
		minutesAgo(30),
	);
	textMessage(
		me,
		aliceChat,
		'Yes! The p2p sync is working great.',
		minutesAgo(28),
	);
	textMessage(
		alice,
		aliceChat,
		'I love that it works offline too',
		minutesAgo(25),
	);
	textMessage(me, aliceChat, 'Absolutely. No servers needed!', minutesAgo(20));
	textMessage(
		alice,
		aliceChat,
		'See you at the meetup tomorrow?',
		minutesAgo(5),
	);

	const bobChat = chatIdForDevices(me, bob);
	textMessage(bob, bobChat, 'Check out this article on E2EE', minutesAgo(60));
	textMessage(me, bobChat, 'Thanks, will read it later!', minutesAgo(55));

	textMessage(
		carol,
		chatIdForDevices(me, carol),
		'Welcome to Dash Chat!',
		minutesAgo(180),
	);

	// A contact request from Eve, not accepted yet.
	new LocalStorageLogsClient(DEMO_CONTACT_REQUESTER.deviceId).createSync(
		DEMO_IDS.INBOX_TOPIC,
		{
			type: 'Inbox',
			payload: {
				type: 'ContactRequest',
				payload: {
					agent_id: DEMO_CONTACT_REQUESTER.agentId,
					reply_topic: DEMO_IDS.INBOX_TOPIC,
					profile: {
						name: DEMO_CONTACT_REQUESTER.name,
						surname: DEMO_CONTACT_REQUESTER.surname,
						avatar: undefined,
						about: undefined,
					},
				},
			},
		},
		minutesAgo(10),
	);

	localStorage.setItem('__preview_seeded', 'true');
}
