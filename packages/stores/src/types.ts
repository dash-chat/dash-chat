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
 * A renderable photo attachment. Carries only the blob `hash` and metadata —
 * never the raw bytes. The bytes live in the iroh-blobs store and are loaded
 * lazily via the `irohblob://` URI scheme (see `mediaSrc` / `loadMediaBytes`).
 */
export interface Photo {
	/** Blob hash of the stored bytes. */
	hash: Hash;
	/** Encoded size in bytes, from the stored metadata. */
	size: number;
	name: string;
	mime_type: string;
}

/** A renderable non-image file attachment. See `Photo` — hash + metadata only. */
export interface FileAttachment {
	hash: Hash;
	size: number;
	name: string;
	mime_type: string;
}

/**
 * Renderable media attached to a chat message. A message has either a set of
 * photos or a single file — not both. Built from a log's `MediaMetaCollection`
 * via `mediaMetaToMedia`; carries hashes, not bytes.
 */
export type Media =
	| { kind: 'photos'; photos: Photo[] }
	| { kind: 'file'; file: FileAttachment };

/**
 * Raw bytes leaving the composer for the backend to store. `data` carries raw
 * bytes — NOT base64; in-process it is a `Uint8Array`, and over Tauri JSON IPC
 * a `Vec<u8>` arrives as `number[]`. This is the *only* media shape that holds
 * bytes: once `store_media` has stored them, all reads go through `irohblob://`.
 * Mirrors `dashchat_node::MediaData`.
 */
export type OutgoingMedia =
	| { kind: 'photos'; photos: OutgoingPhoto[] }
	| { kind: 'file'; file: OutgoingFile };

export interface OutgoingPhoto {
	data: Uint8Array | number[];
	name: string;
	mime_type: string;
}

export interface OutgoingFile {
	data: Uint8Array | number[];
	name: string;
	mime_type: string;
}

export type MediaMetaKind = 'Photo' | 'File';

/**
 * Metadata for a single stored blob. A message log carries these in place of
 * the raw bytes; the bytes live in the iroh-blobs store and are fetched lazily
 * via the `irohblob://` URI scheme. Matches `dashchat_node::MediaMetaItem`.
 */
export interface MediaMetaItem {
	name: string;
	mime_type: string;
	size: number;
	kind: MediaMetaKind;
	hash: Hash;
}

/** Matches `dashchat_node::MediaMetaCollection`, which serializes as a flat array. */
export type MediaMetaCollection = MediaMetaItem[];

/**
 * Convert the blob metadata stored in a message log into the renderable
 * `Media` shape. Mirrors `Node::load_media`: a lone file becomes a `file`
 * attachment, otherwise the items become `photos`. The resulting photos/file
 * carry only a `hash` (no bytes).
 */
export function mediaMetaToMedia(
	meta: MediaMetaCollection | null | undefined,
): Media | null {
	if (!meta || meta.length === 0) return null;
	const file = meta.find(item => item.kind === 'File');
	if (file) {
		return {
			kind: 'file',
			file: {
				name: file.name,
				mime_type: file.mime_type,
				size: file.size,
				hash: file.hash,
			},
		};
	}
	const photos: Photo[] = meta.map(item => ({
		name: item.name,
		mime_type: item.mime_type,
		size: item.size,
		hash: item.hash,
	}));
	return { kind: 'photos', photos };
}

/**
 * V1 (Versioned) form of `ChatMessageContent` — matches the serialization in
 * `crates/dashchat-node/src/chat/message.rs`. Sent messages are always V1.
 * Stored payloads may also appear as a bare string (V0/Unversioned); see
 * `getMessageText` for reading either form.
 */
export type MessageContentV1 = {
	v: '1';
	message: string;
	/** Stored/wire form: a flat `MediaMetaCollection` (bytes live in the blob
	 * store, fetched lazily via `irohblob://`). `getMessageMedia` turns this
	 * into the renderable `Media`. */
	media: MediaMetaCollection | null;
};
export type MessageContent = MessageContentV1;

export function getMessageText(content: MessageContent | string): string {
	return typeof content === 'string' ? content : content.message;
}

export function getMessageMedia(
	content: MessageContent | string,
): Media | null {
	if (typeof content === 'string') return null;
	return mediaMetaToMedia(content.media);
}

/**
 * Cheap structural comparison used to match a just-sent message against the
 * operation that confirms it — media-only messages all have empty text, so
 * text alone cannot disambiguate them. Compares kind plus photo count or
 * file name; byte contents are deliberately not compared (the sent side holds
 * `OutgoingMedia` bytes while the logged side is hash-only `Media`).
 */
export function sameMediaShape(
	a: Media | OutgoingMedia | null,
	b: Media | OutgoingMedia | null,
): boolean {
	if (a === null || b === null) return a === b;
	if (a.kind === 'photos' && b.kind === 'photos') {
		return a.photos.length === b.photos.length;
	}
	if (a.kind === 'file' && b.kind === 'file') {
		return a.file.name === b.file.name;
	}
	return false;
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

export interface ReadMessagesStore {
	markAsRead(messageHashes: Hash[]): Promise<void>;
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
