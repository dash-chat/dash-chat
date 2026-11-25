import type { GroupMember } from '../group-chats/group-chat-client';
import type { ChatId } from '../types';

const MEMBERS_KEY_PREFIX = 'mock-group-members/';

/** Make `chatId` a group chat of the mock backend, with `members`. */
export function setMockGroupMembers(
	chatId: ChatId,
	members: GroupMember[],
): void {
	localStorage.setItem(MEMBERS_KEY_PREFIX + chatId, JSON.stringify(members));
}

export function mockGroupMembers(chatId: ChatId): GroupMember[] {
	const stored = localStorage.getItem(MEMBERS_KEY_PREFIX + chatId);
	return stored === null ? [] : (JSON.parse(stored) as GroupMember[]);
}

export function mockGroupChatIds(): ChatId[] {
	return Object.keys(localStorage)
		.filter(key => key.startsWith(MEMBERS_KEY_PREFIX))
		.map(key => key.slice(MEMBERS_KEY_PREFIX.length));
}
