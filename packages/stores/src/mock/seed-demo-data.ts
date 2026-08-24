import { personalTopicFor } from '../topics';
import { LocalStorageLogsClient, chatIdForDevices } from './client';

export const DEMO_IDS = {
	MY_AGENT_ID: 'aa'.repeat(32),
	MY_DEVICE_ID: 'bb'.repeat(32),
	DEVICE_GROUP_TOPIC: 'cc'.repeat(32),
	INBOX_TOPIC: 'dd'.repeat(32),
};

const CONTACTS = [
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

const EVE = {
	agentId: '05'.repeat(32),
	deviceId: '15'.repeat(32),
	name: 'Eve',
	surname: 'Taylor',
};

/** The device pubkey of each demo identity, for agent → device lookups. */
export const DEMO_CONTACT_DEVICES: Record<string, string> = Object.fromEntries(
	[...CONTACTS, EVE].map(contact => [contact.agentId, contact.deviceId]),
);

/** Seconds-since-epoch timestamps (matching real backend) */
function minutesAgo(minutes: number): number {
	return Math.floor((Date.now() - minutes * 60_000) / 1000);
}

export function seedDemoData(logsClient: LocalStorageLogsClient) {
	if (localStorage.getItem('__preview_seeded')) return;

	// 1. My profile
	logsClient.createSync(
		personalTopicFor(DEMO_IDS.MY_AGENT_ID),
		{
			type: 'Announcements',
			payload: {
				type: 'SetProfile',
				payload: {
					name: 'You',
					surname: undefined,
					avatar: undefined,
					about: 'Testing preview mode',
				},
			},
		},
		minutesAgo(120),
	);

	// 2. Contact profiles (each written by their own device)
	for (const contact of CONTACTS) {
		const contactLogsClient = new LocalStorageLogsClient(contact.deviceId);
		contactLogsClient.createSync(
			personalTopicFor(contact.agentId),
			{
				type: 'Announcements',
				payload: {
					type: 'SetProfile',
					payload: {
						name: contact.name,
						surname: contact.surname,
						avatar: undefined,
						about: contact.about,
					},
				},
			},
			minutesAgo(100),
		);
	}

	// 3. Add contacts to device group
	CONTACTS.forEach((contact, i) => {
		logsClient.createSync(
			DEMO_IDS.DEVICE_GROUP_TOPIC,
			{
				type: 'DeviceGroupPayload',
				payload: {
					type: 'AddContact',
					payload: {
						agent_id: contact.agentId,
						direct_chat_topic_id: chatIdForDevices(
							DEMO_IDS.MY_DEVICE_ID,
							contact.deviceId,
						),
					},
				},
			},
			minutesAgo(90 - i),
		);
	});

	// 4. Chat messages

	// Alice: conversation with several messages
	const aliceChatId = chatIdForDevices(
		DEMO_IDS.MY_DEVICE_ID,
		CONTACTS[0].deviceId,
	);
	const aliceClient = new LocalStorageLogsClient(CONTACTS[0].deviceId);

	aliceClient.createSync(
		aliceChatId,
		{
			type: 'Chat',
			payload: {
				type: 'Message',
				payload: 'Hey! Have you tried the new encrypted messaging?',
			},
		},
		minutesAgo(30),
	);

	logsClient.createSync(
		aliceChatId,
		{
			type: 'Chat',
			payload: {
				type: 'Message',
				payload: 'Yes! The p2p sync is working great.',
			},
		},
		minutesAgo(28),
	);

	aliceClient.createSync(
		aliceChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'I love that it works offline too' },
		},
		minutesAgo(25),
	);

	logsClient.createSync(
		aliceChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'Absolutely. No servers needed!' },
		},
		minutesAgo(20),
	);

	aliceClient.createSync(
		aliceChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'See you at the meetup tomorrow?' },
		},
		minutesAgo(5),
	);

	// Bob: short conversation
	const bobChatId = chatIdForDevices(
		DEMO_IDS.MY_DEVICE_ID,
		CONTACTS[1].deviceId,
	);
	const bobClient = new LocalStorageLogsClient(CONTACTS[1].deviceId);

	bobClient.createSync(
		bobChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'Check out this article on E2EE' },
		},
		minutesAgo(60),
	);

	logsClient.createSync(
		bobChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'Thanks, will read it later!' },
		},
		minutesAgo(55),
	);

	// Carol: single message
	const carolChatId = chatIdForDevices(
		DEMO_IDS.MY_DEVICE_ID,
		CONTACTS[2].deviceId,
	);
	const carolClient = new LocalStorageLogsClient(CONTACTS[2].deviceId);

	carolClient.createSync(
		carolChatId,
		{
			type: 'Chat',
			payload: { type: 'Message', payload: 'Welcome to Dash Chat!' },
		},
		minutesAgo(180),
	);

	// Dave: no messages (contact only)

	// 5. Contact request from Eve (inbox message, not yet accepted)
	const eveClient = new LocalStorageLogsClient(EVE.deviceId);
	eveClient.createSync(
		DEMO_IDS.INBOX_TOPIC,
		{
			type: 'Inbox',
			payload: {
				type: 'ContactRequest',
				payload: {
					agent_id: EVE.agentId,
					reply_topic: DEMO_IDS.INBOX_TOPIC,
					profile: {
						name: EVE.name,
						surname: EVE.surname,
						avatar: undefined,
						about: undefined,
					},
				},
			},
		},
		minutesAgo(10),
	);

	// 6. Mark as seeded
	localStorage.setItem('__preview_seeded', 'true');
}
