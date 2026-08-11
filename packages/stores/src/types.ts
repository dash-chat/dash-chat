import { Profile } from './contacts/contacts-client';
import {
	AgentId,
	DeviceId,
	Hash,
	TopicId,
	VerifyingKey,
} from './p2panda/types';

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
 * V1 (Versioned) form of `ChatMessageContent` — matches the serialization in
 * `crates/dashchat-node/src/chat/message.rs`. Sent messages are always V1.
 */
export type MessageContentV1 = {
	v: '1';
	message: string;
	/** Stored/wire form: a flat `MediaBundle` (bytes live in the blob store,
	 * fetched lazily via `irohblob://`). Consumers derive the photos/file/voice
	 * grouping from this list at render time. */
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

export interface EditMessagePayload {
	/** The corrected text. Media cannot be edited. */
	message: string;
	/** Hash of the message (or prior edit) being edited; edits chain linearly. */
	edit_hash: Hash;
}

export interface DeleteMessagePayload {
	/** The complete edit chain being deleted: the original message plus every
	 * edit (a single hash when the message was never edited). */
	hashes: Hash[];
}

export type ChatPayload =
	| { type: 'Message'; payload: MessageContent }
	| { type: 'Reaction'; payload: ChatReaction }
	| { type: 'EditMessage'; payload: EditMessagePayload }
	| { type: 'DeleteMessage'; payload: DeleteMessagePayload }
	| { type: 'JoinGroup'; payload: { chat_id: string } }
	| { type: 'GroupInfo'; payload: GroupInfo };

export interface ReadMessagesPayload {
	chat_id: ChatId;
	message_hashes: Hash[];
}

export type DeviceGroupPayload =
	| { type: 'AddContact'; payload: { agent_id: AgentId } }
	| {
			type: 'PendingContactRequest';
			payload: { device_pubkey: DeviceId; profile_name: string };
	  }
	| { type: 'RejectContactRequest'; payload: AgentId }
	| { type: 'BlockAgent'; payload: AgentId }
	| { type: 'UnblockAgent'; payload: AgentId }
	| { type: 'ReadMessages'; payload: ReadMessagesPayload };

export type InboxPayload = {
	type: 'ContactRequest';
	payload: {
		profile: Profile;
		agent_id: AgentId;
		reply_topic: TopicId;
	};
};

/** `p2panda_auth::processor::GroupsArgs`; `action` is not modeled here. */
export interface GroupControlPayload {
	group_id: VerifyingKey;
	dependencies: Hash[];
}

export type Payload =
	| { type: 'Announcements'; payload: AnnouncementPayload }
	| { type: 'Chat'; payload: ChatPayload }
	| { type: 'DeviceGroupPayload'; payload: DeviceGroupPayload }
	| { type: 'Inbox'; payload: InboxPayload }
	| { type: 'GroupControl'; payload: GroupControlPayload };

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

export type BlockEvent = {
	kind: 'contact_blocked' | 'contact_unblocked';
	contactName: string | undefined;
	timestamp: number;
};

/** A single version of a message's text, with the time it was authored. */
export interface MessageVersion {
	hash: string;
	text: string;
	timestamp: number;
}

/** The live, renderable body of a message: its text, media, reactions and edit
 * history. */
export interface MessageBody {
	message: string;
	media: MediaBundle | null;
	reactions: Record<DeviceId, string>;
	editHistory: MessageVersion[];
}

/** The renderable content of a message, or `'deleted-for-everyone'` once a
 * delete op replaces the body entirely — dropping text, media, reactions and
 * edits — and it renders as the deleted placeholder. */
export type MessageDisplay = MessageBody | 'deleted-for-everyone';

/** Whether a message still has a live body. Written as a type guard so the
 * `true` branch narrows `content` to `MessageBody`. */
export function hasBody(content: MessageDisplay): content is MessageBody {
	return typeof content !== 'string';
}

/** Whether a message was deleted for everyone. */
export function isDeleted(
	content: MessageDisplay,
): content is 'deleted-for-everyone' {
	return content === 'deleted-for-everyone';
}

export type ChatSummaryLastEvent =
	| {
			kind: 'message';
			content: MessageDisplay;
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
