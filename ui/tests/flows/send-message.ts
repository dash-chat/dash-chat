import { S } from '../selectors';

/**
 * Send and verify message delivery flow.
 *
 * Precondition: Both agents are contacts and have a direct chat.
 *
 * Steps:
 *   1. On Agent 1: Open the direct chat with Agent 2
 *      - Click the chat in S.home.chatList, or navigate to /direct-chats/{agentId}
 *
 *   2. On Agent 1: Send a message
 *      - Type into: S.messageInput.textarea
 *      - Click: S.messageInput.send
 *
 *   3. On Agent 2: Open the direct chat with Agent 1
 *
 *   4. On Agent 2: Verify the message appears
 *      - Wait for: S.directChat.messages
 *      - Check message content via JS:
 *        document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes('...')
 */

export const steps = {
	messageInput: S.messageInput.textarea,
	sendButton: S.messageInput.send,
	messagesContainer: S.directChat.messages,
	verifyMessageScript: (text: string) =>
		`document.querySelector('${S.directChat.messages}')?.textContent?.includes('${text.replace(/'/g, "\\'")}')`,
};
