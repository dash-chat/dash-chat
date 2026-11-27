import type { ChatId, GroupMember } from 'dash-chat-stores';
import Emittery, { type UnsubscribeFunction } from 'emittery';

const MEMBERS_KEY_PREFIX = 'mock-group-members/';

const groupChanges = new Emittery<{
	'group-chat-added': ChatId;
	'members-changed': ChatId;
}>();

/** Make `chatId` a group chat of the mock backend, with `members`. */
export function setMockGroupMembers(
	chatId: ChatId,
	members: GroupMember[],
): void {
	const isNew = localStorage.getItem(MEMBERS_KEY_PREFIX + chatId) === null;
	localStorage.setItem(MEMBERS_KEY_PREFIX + chatId, JSON.stringify(members));
	void groupChanges.emit(
		isNew ? 'group-chat-added' : 'members-changed',
		chatId,
	);
}

export function onMockGroupChatAdded(
	handler: (chatId: ChatId) => void,
): UnsubscribeFunction {
	return groupChanges.on('group-chat-added', handler);
}

export function onMockGroupMembersChanged(
	chatId: ChatId,
	handler: () => void,
): UnsubscribeFunction {
	return groupChanges.on('members-changed', changed => {
		if (changed === chatId) handler();
	});
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
