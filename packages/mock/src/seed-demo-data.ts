import {
	type AgentId,
	type ChatId,
	type ChatPayload,
	type DeviceGroupPayload,
	type DeviceId,
	type Hash,
	type Profile,
	personalTopicFor,
} from 'dash-chat-stores';

import { backend, chatIdForDevices } from './backend.ts';
import { DEMO_CONTACTS, DEMO_CONTACT_REQUESTER, DEMO_IDS } from './demo-ids.ts';

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
	return backend.write(author, chatId, { type: 'Chat', payload }, timestamp);
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
	return backend.write(
		DEMO_IDS.MY_DEVICE_ID,
		DEMO_IDS.DEVICE_GROUP_TOPIC,
		{ type: 'DeviceGroupPayload', payload },
		timestamp,
	);
}

/** Announce `profile` as `person`'s. */
export function profileOperation(
	person: Person,
	profile: Profile,
	timestamp?: number,
): Hash {
	return backend.write(
		person.deviceId,
		personalTopicFor(person.agentId),
		{
			type: 'Announcements',
			payload: { type: 'SetProfile', payload: profile },
		},
		timestamp,
	);
}

export function seedDemoData() {
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
	backend.write(
		DEMO_CONTACT_REQUESTER.deviceId,
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
}
