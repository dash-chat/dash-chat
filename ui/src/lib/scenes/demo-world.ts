import type { Page } from '@playwright/test';
import {
	DEMO_CONTACTS,
	DEMO_IDS,
	LocalStorageLogsClient,
	chatIdForDevices,
	chatOperation,
	minutesAgo,
	profileOperation,
	removeDemoData,
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
	page: Page,
	files: { name: string; mimeType: string; bytes?: number[]; size?: number }[],
): Promise<void> {
	await page.getByTestId('message-input-textarea').first().waitFor();
	await page.evaluate(pasted => window.__test.pasteFiles(pasted), files);
}

/** Start from a device nobody has set up, not from the demo data. */
export function nobodyHasSetUpThisDevice(): void {
	removeDemoData();
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
export async function reportingIsUnavailable(page: Page): Promise<void> {
	await page.addInitScript(() =>
		Object.assign(globalThis, { __reportingIsUnavailable: true }),
	);
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
