import {
	DEMO_CONTACTS,
	DEMO_IDS,
	LocalStorageLogsClient,
	chatIdForDevices,
	chatOperation,
	profileOperation,
	setMockGroupMembers,
} from 'dash-chat-mock';
import {
	type AgentId,
	type AuthGroupMember,
	type ChatId,
	type DeviceId,
	type GroupAction,
	type Hash,
} from 'dash-chat-stores';
import { find } from 'i18n-screenshots/scene';

export { TINY_PNG_BYTES } from '../../../tests/fixtures';

export const [ALICE, BOB, CAROL, DAVE] = DEMO_CONTACTS;

export function directChatId(contact: { deviceId: DeviceId }): ChatId {
	return chatIdForDevices(DEMO_IDS.MY_DEVICE_ID, contact.deviceId);
}

export function directChatPath(contact: { deviceId: DeviceId }): string {
	return `/direct-chats/${directChatId(contact)}`;
}

export const STRANGER = { agentId: '09'.repeat(32), deviceId: '19'.repeat(32) };

/** Stage `files` in the composer the way a paste does. */
export async function pasteIntoComposer(
	files: { name: string; mimeType: string; bytes?: number[]; size?: number }[],
): Promise<void> {
	await find('message-input-textarea');
	window.__test.pasteFiles(files);
}

/** Start from a device nobody has set up, not from the demo data. */
export function nobodyHasSetUpThisDevice(): void {
	localStorage.clear();
}

/** The only thing on the device: my own profile, as right after onboarding. */
export function onlyMyProfile(): void {
	nobodyHasSetUpThisDevice();
	profileOperation(ME, {
		name: 'You',
		surname: undefined,
		avatar: undefined,
		about: undefined,
	});
}

/** Make this a build with nowhere to send error reports to. */
export function reportingIsUnavailable(): void {
	Object.assign(globalThis, { __reportingIsUnavailable: true });
}

export const ME = {
	agentId: DEMO_IDS.MY_AGENT_ID,
	deviceId: DEMO_IDS.MY_DEVICE_ID,
};

export const GROUP_CHAT_ID: ChatId = 'ee'.repeat(32);
export const GROUP_CHAT_PATH = `/group-chat/${GROUP_CHAT_ID}`;

interface Person {
	agentId: AgentId;
	deviceId: DeviceId;
}

export function individual(person: Person): AuthGroupMember {
	return { Individual: person.deviceId };
}

/** Write a group-control operation as `author`: creating the demo group,
 * adding or removing a member, promoting or demoting one. */
export function groupOperation(
	author: Person,
	action: GroupAction,
	timestamp: number,
): Hash {
	return new LocalStorageLogsClient(author.deviceId).createSync(
		GROUP_CHAT_ID,
		undefined,
		timestamp,
		{ group_id: GROUP_CHAT_ID, action, dependencies: [] },
	);
}

/** Name the demo group and say who is in it now. */
export function describeGroup(
	name: string,
	members: Person[],
	admins: Person[],
	timestamp: number,
): void {
	chatOperation(
		admins[0].deviceId,
		GROUP_CHAT_ID,
		{
			type: 'GroupInfo',
			payload: { name, description: undefined, image: undefined },
		},
		timestamp,
	);
	setMockGroupMembers(
		GROUP_CHAT_ID,
		members.map(member => ({
			agentId: member.agentId,
			deviceIds: [member.deviceId],
			isAdmin: admins.includes(member),
		})),
	);
}
