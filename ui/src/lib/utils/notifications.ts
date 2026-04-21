import { goto } from '$app/navigation';
import { invoke } from '@tauri-apps/api/core';
import type { Options } from '@tauri-apps/plugin-notification';
import { onAction } from '@tauri-apps/plugin-notification';
import type { ChatsStore, ContactsStore } from 'dash-chat-stores';

async function navigateToChat(
	chatsStore: ChatsStore,
	contactsStore: ContactsStore,
	notification: Options,
) {
	const topicId = notification.group;
	if (!topicId) return;

	// Check if this is an inbox topic (contact request)
	const inboxTopics = await contactsStore.client.activeInboxTopics();
	if (inboxTopics.includes(topicId)) {
		const contactRequests = await contactsStore.contactRequests();
		const match = contactRequests.find(cr => cr.topicId === topicId);
		if (match) {
			goto(`/direct-chats/${match.code.agent_id}`);
		}
		return;
	}

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

export function setupNotificationNavigation(
	chatsStore: ChatsStore,
	contactsStore: ContactsStore,
) {
	// Handle taps on notifications while the app is running
	onAction(async notificationWithAction => {
		const notification: Options = (notificationWithAction as any).notification;
		if (notification) {
			navigateToChat(chatsStore, contactsStore, notification);
		}
	});

	// Handle the notification that launched the app (cold start)
	invoke<{ notification: Options } | null>(
		'plugin:notification|get_launching_notification_action',
	)
		.then(payload => {
			if (payload?.notification) {
				navigateToChat(chatsStore, contactsStore, payload.notification);
			}
		})
		.catch(error => {
			console.error('Failed to get_launching_notification_action', error);
		});
}
