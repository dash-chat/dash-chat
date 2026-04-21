import { goto } from '$app/navigation';
import { invoke } from '@tauri-apps/api/core';
import type { Options } from '@tauri-apps/plugin-notification';
import { onAction } from '@tauri-apps/plugin-notification';
import type { ChatsStore } from 'dash-chat-stores';

import { isMobile } from './environment';

async function navigateToChat(chatsStore: ChatsStore, notification: Options) {
	const topicId = notification.group;
	if (!topicId) return;

	const chatIds = await chatsStore.allChatsIds();
	for (const agentId of chatIds) {
		const directChatId = await chatsStore
			.directChats(agentId)
			.client.chatId(agentId);
		if (directChatId === topicId) {
			goto(`/direct-chats/${agentId}`);
			return;
		}
	}

	goto(`/group-chat/${topicId}`);
}

export function setupNotificationNavigation(chatsStore: ChatsStore) {
	// Handle taps on notifications while the app is running
	onAction(async notificationWithAction => {
		const notification: Options = (notificationWithAction as any).notification;
		if (notification) {
			navigateToChat(chatsStore, notification);
		}
	});

	// Handle the notification that launched the app (cold start)
	invoke<{ notification: Options } | null>(
		'plugin:notification|get_launching_notification_action',
	)
		.then(payload => {
			if (payload?.notification) {
				navigateToChat(chatsStore, payload.notification);
			}
		})
		.catch(error => {
			console.error('Failed to get_launching_notification_action', error);
		});
}
