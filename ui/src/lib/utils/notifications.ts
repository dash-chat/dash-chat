import { goto } from '$app/navigation';
import type { ChatsStore } from 'dash-chat-stores';

export function setupNotificationNavigation(chatsStore: ChatsStore) {
	import('@tauri-apps/plugin-notification').then(({ onAction }) => {
		onAction(async (notification) => {
			const topicId = notification.group;
			if (!topicId) return;

			const chatIds = await chatsStore.allChatsIds();
			for (const agentId of chatIds) {
				const directChatId = await chatsStore.directChats(agentId).client.chatId(agentId);
				if (directChatId === topicId) {
					goto(`/direct-chats/${agentId}`);
					return;
				}
			}

			goto(`/group-chat/${topicId}`);
		});
	});
}
