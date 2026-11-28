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
