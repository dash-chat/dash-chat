import { Profile } from './contacts/contacts-client';
import { AgentId, DeviceId, Hash, TopicId } from './p2panda/types';

export type ChatId = TopicId;

export interface SpaceControlMessage {
	hash: Hash;
	author: AgentId;
	timestamp: number;
	// spaces_args: SpacesArgs,
}

export interface ChatReaction {
	/// The emoji to react with.
	/// Use None to "remove" the prior reaction.
	emoji: string | null;
	/// The hash of the header of the message being reacted to.
	target: Hash;
}

/**
 * `data` is the raw attachment bytes. The Rust side holds a `Vec<u8>`, which
 * the Tauri JSON IPC delivers to the webview as a `number[]`; `getMessageMedia`
 * materializes that into a `Uint8Array` at the boundary, so every consumer (and
 * the send path, which builds `Uint8Array` directly) works with `Uint8Array`.
 */
export interface Photo {
	data: Uint8Array;
	name: string;
	mime_type: string;
}

/** A non-image file attachment. See `Photo` for the `data` shape. */
export interface FileAttachment {
	data: Uint8Array;
	name: string;
	mime_type: string;
}

/**
 * Media attached to a chat message. A message has either a set of photos
 * or a single file — not both. Matches `dashchat_node::Media`.
 */
export type Media =
	| { kind: 'photos'; photos: Photo[] }
	| { kind: 'file'; file: FileAttachment };

/**
 * V1 (Versioned) form of `ChatMessageContent` — matches the serialization in
 * `crates/dashchat-node/src/chat/message.rs`. Sent messages are always V1.
 */
export type MessageContentV1 = {
	v: '1';
	message: string;
	media: Media | null;
};
export type MessageContent = MessageContentV1;

export function getMessageText(content: MessageContent): string {
	return content.message;
}

export function getMessageMedia(content: MessageContent): Media | null {
	if (!content.media) return null;
	const media = content.media;
	// The Tauri JSON IPC delivers `Vec<u8>` as `number[]`; rebuild it as a
	// `Uint8Array` here so consumers never deal with the raw wire form.
	if (media.kind === 'photos') {
		return {
			kind: 'photos',
			photos: media.photos.map(p => ({ ...p, data: new Uint8Array(p.data) })),
		};
	}
	return {
		kind: 'file',
		file: { ...media.file, data: new Uint8Array(media.file.data) },
	};
}

/**
 * Short single-line description of a message for chat list previews. Falls
 * back to a media descriptor when the text is empty.
 */
export function summarizeMessageContent(content: {
	message: string;
	media: Media | null;
}): string {
	if (content.message) return content.message;
	if (!content.media) return '';
	if (content.media.kind === 'file') return content.media.file.name;
	const n = content.media.photos.length;
	return n > 1 ? `${n} photos` : 'Photo';
}

export type AnnouncementPayload =
	| { type: 'SetProfile'; payload: Profile }
	| { type: 'SetCapabilities'; payload: unknown };
export interface GroupInfo {
	name: string;
	description: string | undefined;
	image: string | undefined;
}

export type ChatPayload =
	| { type: 'Message'; payload: MessageContent }
	| { type: 'Reaction'; payload: ChatReaction }
	| { type: 'JoinGroup'; payload: { chat_id: string } }
	| { type: 'GroupInfo'; payload: GroupInfo };

export interface InboxTopic {
	expires_at: number;
	topic: TopicId;
}

export type ShareIntent = 'AddDevice' | 'AddContact';

export interface ContactCode {
	/// Pubkey of this node: allows adding this node to groups.
	device_pubkey: DeviceId;
	/// Agent ID to add to spaces
	agent_id: AgentId;
	inbox_topic: InboxTopic | undefined;
	/// The intent of the QR code: whether to add this node as a contact or a device.
	share_intent: ShareIntent;
}

export interface ReadMessagesPayload {
	chat_id: ChatId;
	message_hashes: Hash[];
}

export type DeviceGroupPayload =
	| { type: 'AddContact'; payload: ContactCode }
	| { type: 'RejectContactRequest'; payload: AgentId }
	| { type: 'ReadMessages'; payload: ReadMessagesPayload };

export type InboxPayload = {
	type: 'ContactRequest';
	payload: {
		code: ContactCode;
		profile: Profile;
	};
};

export type Payload =
	| { type: 'Announcements'; payload: AnnouncementPayload }
	| { type: 'Chat'; payload: ChatPayload }
	| { type: 'DeviceGroupPayload'; payload: DeviceGroupPayload }
	| { type: 'Inbox'; payload: InboxPayload };

export type MessageId = string;

// export type MessageContent = {
// 	type: 'TextMessage';
// 	message: string;
// 	replyTo: MessageId | undefined;
// };

// export interface Message {
// 	id: MessageId;
// 	content: MessageContent;
// 	author: VerifyingKey;
// 	timestamp: number;
// }

export interface MessagesStore {
	markAsRead(messageHashes: Hash[]): Promise<void>;
	/** Sends the message and resolves with the operation id of the created
	 * message once it is confirmed in the local log. */
	sendMessage(input: { message: string; media: Media | null }): Promise<Hash>;
}

export type GroupControlEvent =
	| {
			kind: 'group_created';
			isMine: boolean;
			iAmInitialMember: boolean;
			creatorName: string | undefined;
			timestamp: number;
	  }
	| {
			kind: 'group_member_added';
			isMine: boolean;
			addedByMe: boolean;
			memberName: string | undefined;
			adminName: string | undefined;
			timestamp: number;
	  }
	| {
			kind: 'group_member_removed';
			isMine: boolean;
			removedByMe: boolean;
			memberName: string | undefined;
			adminName: string | undefined;
			timestamp: number;
	  }
	| {
			kind: 'group_member_promoted';
			promotedByMe: boolean;
			memberName: string | undefined;
			adminName: string | undefined;
			timestamp: number;
	  }
	| {
			kind: 'group_member_demoted';
			demotedByMe: boolean;
			memberName: string | undefined;
			adminName: string | undefined;
			timestamp: number;
	  };

export type ChatSummaryLastEvent =
	| {
			kind: 'message';
			text: string;
			authorName?: string;
			timestamp: number;
	  }
	| { kind: 'contact_request'; timestamp: number }
	| { kind: 'contact_added'; timestamp: number }
	| GroupControlEvent;

export interface ChatSummary {
	type: 'GroupChat' | 'DirectChat';
	chatId: TopicId;
	unreadMessages: number;
	name: string;
	avatar: string | undefined;
	lastEvent: ChatSummaryLastEvent;
}
