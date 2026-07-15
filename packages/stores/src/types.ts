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
export interface PhotoAttachment {
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
export type MediaAttachment =
	| { kind: 'photos'; photos: PhotoAttachment[] }
	| { kind: 'file'; file: FileAttachment };

/**
 * Raw bytes leaving the composer for the backend to store. `data` carries raw
 * bytes — NOT base64; in-process it is a `Uint8Array`, and over Tauri JSON IPC
 * a `Vec<u8>` arrives as `number[]`. This is the *only* media shape that holds
 * bytes: once `store_media` has stored them, all reads go through `irohblob://`.
 * Mirrors `dashchat_node::OutgoingMedia`.
 */
export type OutgoingMedia =
	| { kind: 'photos'; photos: OutgoingPhoto[] }
	| { kind: 'file'; file: OutgoingFile };

export interface OutgoingPhoto {
	data: Uint8Array;
	name: string;
	mime_type: string;
}

export interface OutgoingFile {
	data: Uint8Array;
	name: string;
	mime_type: string;
}

export type MediaMetaKind = 'Photo' | 'File';

/**
 * Metadata for a single stored blob. A message log carries these in place of
 * the raw bytes; the bytes live in the iroh-blobs store and are fetched lazily
 * via the `irohblob://` URI scheme. Matches `dashchat_node::MediaMetaItem`.
 */
export interface MediaMetadata {
	name: string;
	mime_type: string;
	size: number;
	kind: MediaMetaKind;
	hash: Hash;
}

/** Matches `dashchat_node::MediaBundle`, which serializes as a flat array. */
export type MediaBundle = MediaMetadata[];

/**
 * Convert the blob metadata stored in a message log into the renderable
 * `MediaAttachment` shape. Mirrors `Node::load_media`: a lone file becomes a `file`
 * attachment, otherwise the items become `photos`. The resulting photos/file
 * carry only a `hash` (no bytes).
 */
export function mediaBundleToAttachment(
	meta: MediaBundle | null | undefined,
): MediaAttachment | null {
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
	const photos: PhotoAttachment[] = meta.map(item => ({
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
 */
export type MessageContentV1 = {
	v: '1';
	message: string;
	/** Stored/wire form: a flat `MediaBundle` (bytes live in the blob
	 * store, fetched lazily via `irohblob://`). `mediaBundleToAttachment` turns this
	 * into the renderable `MediaAttachment`. */
	media: MediaBundle | null;
};
export type MessageContent = MessageContentV1;

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

export interface ReadMessagesPayload {
	chat_id: ChatId;
	message_hashes: Hash[];
}

export type DeviceGroupPayload =
	| { type: 'AddContact'; payload: { agent_id: AgentId } }
	| { type: 'PendingContactRequest'; payload: { device_pubkey: DeviceId } }
	| { type: 'RejectContactRequest'; payload: AgentId }
	| { type: 'ReadMessages'; payload: ReadMessagesPayload };

export type InboxPayload = {
	type: 'ContactRequest';
	payload: {
		profile: Profile;
		agent_id: AgentId;
		reply_topic: TopicId;
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
			content: { message: string; media: MediaAttachment | null };
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
	waitingForProfile?: true;
}
