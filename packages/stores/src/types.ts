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
 * A renderable voice note. Like `PhotoAttachment` it carries only the blob
 * `hash` and metadata — the audio (a self-contained 16 kHz mono WAV) lives in
 * the iroh-blobs store and is loaded lazily via the `irohblob://` URI scheme.
 * `waveform` holds downsampled, peak-normalized amplitude bars (0..=255) for the
 * scrubber UI, carried in the message metadata so it renders before the audio
 * downloads.
 */
export interface VoiceNote {
	hash: Hash;
	mime_type: string;
	duration_ms: number;
	waveform: Uint8Array;
}

/**
 * Renderable media attached to a chat message: a set of photos, a single file,
 * or a single voice note — never a mix. Built from a log's `MediaBundle` via
 * `mediaBundleToAttachment`; carries hashes, not bytes.
 */
export type MediaAttachment =
	| { kind: 'photos'; photos: PhotoAttachment[] }
	| { kind: 'file'; file: FileAttachment }
	| { kind: 'voice_note'; voice_note: VoiceNote };

/**
 * Raw bytes leaving the composer for the backend to store. `data` carries raw
 * bytes — NOT base64; in-process it is a `Uint8Array`, and over Tauri JSON IPC
 * a `Vec<u8>` arrives as `number[]`. This is the *only* media shape that holds
 * bytes: once `store_media` has stored them, all reads go through `irohblob://`.
 * Mirrors `dashchat_node::OutgoingMedia`.
 */
export type OutgoingMedia =
	| { kind: 'photos'; photos: OutgoingPhoto[] }
	| { kind: 'file'; file: OutgoingFile }
	| { kind: 'voice_note'; voice_note: OutgoingVoiceNote };

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

export interface OutgoingVoiceNote {
	data: Uint8Array;
	mime_type: string;
	duration_ms: number;
	waveform: Uint8Array;
}

/**
 * Metadata for a single stored blob. A message log carries these in place of
 * the raw bytes; the bytes live in the iroh-blobs store and are fetched lazily
 * via the `irohblob://` URI scheme. Mirrors the `#[serde(tag = "kind")]` enum
 * `dashchat_node::MediaMetadata`: photos/files reference their blob by `hash`,
 * while voice notes also carry `duration_ms`/`waveform` so the scrubber renders
 * without first fetching the audio.
 */
export type MediaMetadata =
	| { kind: 'Photo'; name: string; mime_type: string; size: number; hash: Hash }
	| { kind: 'File'; name: string; mime_type: string; size: number; hash: Hash }
	| {
			kind: 'VoiceNote';
			mime_type: string;
			size: number;
			duration_ms: number;
			waveform: Uint8Array;
			hash: Hash;
	  };

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
	const voiceNote = meta.find(item => item.kind === 'VoiceNote');
	if (voiceNote?.kind === 'VoiceNote') {
		return {
			kind: 'voice_note',
			voice_note: {
				hash: voiceNote.hash,
				mime_type: voiceNote.mime_type,
				duration_ms: voiceNote.duration_ms,
				waveform: voiceNote.waveform,
			},
		};
	}
	const file = meta.find(item => item.kind === 'File');
	if (file?.kind === 'File') {
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
	const photos: PhotoAttachment[] = [];
	for (const item of meta) {
		if (item.kind !== 'Photo') continue;
		photos.push({
			name: item.name,
			mime_type: item.mime_type,
			size: item.size,
			hash: item.hash,
		});
	}
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
	sendMessage(input: {
		message: string;
		media: OutgoingMedia | null;
	}): Promise<Hash>;
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
}
