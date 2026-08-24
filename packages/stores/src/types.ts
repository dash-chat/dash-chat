import { Bodyless, Message } from './chats/messages-store';
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

export interface VoiceNote {
	hash: Hash;
	mime_type: string;
	duration_ms: number;
	waveform: Uint8Array;
}

/**
 * Renderable media attached to a chat message. A message has either a set of
 * photos, a single file, or a single voice note. Built from a log's
 * `MediaBundle` via `mediaBundleToAttachment`; carries hashes, not bytes.
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
 * `dashchat_node::MediaMetadata`: each variant carries only what its kind needs.
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
	const voiceNote = meta.find(item => item.kind === 'VoiceNote');
	if (voiceNote) {
		return {
			kind: 'voice_note',
			voice_note: {
				mime_type: voiceNote.mime_type,
				duration_ms: voiceNote.duration_ms,
				waveform: voiceNote.waveform,
				hash: voiceNote.hash,
			},
		};
	}
	const photos: PhotoAttachment[] = meta
		.filter(item => item.kind === 'Photo')
		.map(item => ({
			name: item.name,
			mime_type: item.mime_type,
			size: item.size,
			hash: item.hash,
		}));
	return photos.length > 0 ? { kind: 'photos', photos } : null;
}

/**
 * V1 (Versioned) form of `ChatMessageContent` — matches the serialization in
 * `crates/dashchat-node/src/chat/message.rs`. Sent messages are always V1.
 */
export type MessageContentV1 = {
	message: string;
	/** Stored/wire form: a flat `MediaBundle` (bytes live in the blob store,
	 * fetched lazily via `irohblob://`). Consumers derive the photos/file
	 * grouping from this list at render time. */
	media: MediaBundle | null;
	/** Hash of the operation this message replies to (a `Message` or
	 * `EditMessage` in the same chat, any author's log). Absent on the wire
	 * for non-replies. */
	reply?: Hash;
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

export interface DeleteForMePayload {
	chat_id: ChatId;
	/** The original message being deleted. Its edit chain (present and future)
	 * is tombstoned transitively by the backend, so only the root is named
	 * here — unlike `DeleteMessagePayload`, which lists the whole chain. */
	message_hash: Hash;
}

/** Why an operation was tombstoned, mirroring the backend `TombstoneReason`. A
 * delete-for-everyone still shows a "deleted" placeholder for the author's
 * message; a delete-for-me vanishes with no trace. */
export type TombstoneReason = 'DeletedForEveryone' | 'DeletedForMe';

export type SystemEvent = {
	type: 'Tombstones';
	payload: {
		topic: TopicId;
		hashes: Hash[];
		reason: TombstoneReason;
	};
};

export interface Tombstone {
	hash: Hash;
	reason: TombstoneReason;
}

export type Tombstones = Record<Hash, TombstoneReason>;

export type DeviceGroupPayload =
	| { type: 'AddContact'; payload: { agent_id: AgentId } }
	| {
			type: 'PendingContactRequest';
			payload: { device_pubkey: DeviceId; profile_name: string };
	  }
	| { type: 'RejectContactRequest'; payload: AgentId }
	| { type: 'BlockAgent'; payload: AgentId }
	| { type: 'UnblockAgent'; payload: AgentId }
	| { type: 'ReadMessages'; payload: ReadMessagesPayload }
	| { type: 'DeleteForMe'; payload: DeleteForMePayload }
	| { type: 'ReportContact'; payload: ReportContactPayload };

/** Written after at least one mailbox accepted a report of `agent_id`. */
export interface ReportContactPayload {
	agent_id: AgentId;
	device_ids: DeviceId[];
	mailbox_ids: string[];
}

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
	reactions: Record<AgentId, string>;
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

export function isMessage(message: Message | Bodyless): message is Message {
	return 'content' in message;
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
