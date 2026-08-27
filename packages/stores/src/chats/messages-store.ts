import { ReactivePromise, reactive } from 'signalium';

import { fullName } from '../contacts/contacts-client';
import { ContactsStore } from '../contacts/contacts-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash } from '../p2panda/types';
import { TombstoneStore } from '../tombstones/tombstone-store';
import {
	ChatId,
	ChatReaction,
	MessageDisplay,
	MessageVersion,
	OutgoingMedia,
	Payload,
	Tombstones,
	hasBody,
	isMessage,
	mediaBundleToAttachment,
} from '../types';
import { type IMessagesClient } from './messages-client';
import { type MessageReply } from './replies';

/** The window during which a message may be edited, measured from the original
 * message timestamp. Frontend operation timestamps are milliseconds since the
 * UNIX epoch (the backend serializes them as such), so this is 24h in ms. */
export const EDIT_WINDOW_MS = 24 * 60 * 60 * 1000;

/** The window during which a message may be deleted for everyone, measured
 * from the original message timestamp. Deleting a message for yourself is
 * always allowed. Mirrors `DELETE_WINDOW_MICROS` in
 * `crates/dashchat-node/src/chat/validation/delete.rs` (frontend timestamps
 * are ms). */
export const DELETE_FOR_EVERYONE_WINDOW_MS = 24 * 60 * 60 * 1000;

export interface Message {
	hash: string;
	/** The message payload with its reactions and edit history, or
	 * `'deleted-for-everyone'` once deleted. */
	content: MessageDisplay;
	timestamp: number;
	author: DeviceId;
	seqNum: number;
	replyQuote?: MessageReply;
}

export type Bodyless = {
	hash: string;
	timestamp: number;
	author: DeviceId;
	seqNum: number;
};

// The messages of a single chat, direct or group alike: the message log with
// reactions and read-tracking, plus the actions to publish into it.
export class MessagesStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		protected contactsStore: ContactsStore,
		protected tombstoneStore: TombstoneStore,
		public chatId: ChatId,
		public client: IMessagesClient,
		public readOnly: () => ReactivePromise<boolean>,
	) {}

	messages = reactive(async () => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);

		const opsOrdered = Object.values(logs)
			.flat()
			.sort(
				(a, b) =>
					a.header.timestamp - b.header.timestamp ||
					a.hash.localeCompare(b.hash),
			);
		const tombstones = await this.tombstoneStore.tombstones(this.chatId);
		const deviceAgents = await this.contactsStore.agentsForDevices(
			new Set(Object.keys(logs)),
		);
		const deviceNames = await this.deviceNames();
		const messages = logsToMessages(
			opsOrdered,
			tombstones,
			deviceAgents,
			deviceNames,
		);
		return messages;
	});

	/** Profile name of each device that has authored in this chat, for the
	 * devices whose author's profile is known. */
	deviceNames = reactive(async (): Promise<Record<DeviceId, string>> => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const deviceAgents = await this.contactsStore.agentsForDevices(
			new Set(Object.keys(logs)),
		);
		const profiles = await this.contactsStore.profilesForAgents(
			new Set(Object.values(deviceAgents)),
		);
		const deviceNames: Record<DeviceId, string> = {};
		for (const [deviceId, agentId] of Object.entries(deviceAgents)) {
			const profile = profiles[agentId];
			if (profile) deviceNames[deviceId] = fullName(profile);
		}
		return deviceNames;
	});

	members = reactive(async (): Promise<Array<AgentId>> => {
		const logs = await this.logsStore.logsForAllAuthors(this.chatId);
		const deviceAgents = await this.contactsStore.agentsForDevices(
			new Set(Object.keys(logs)),
		);
		return Array.from(new Set(Object.values(deviceAgents)));
	});

	membersProfiles = reactive(async () => {
		const members = await this.members();
		return await this.contactsStore.profilesForAgents(new Set(members));
	});

	lastMessage = reactive(async () => {
		const messages = await this.messages();

		const sortedMessages = Object.values(messages).sort(
			(m1, m2) => m2.timestamp - m1.timestamp,
		);
		return sortedMessages.length > 0 ? sortedMessages[0] : undefined;
	});

	readMessageHashes = reactive(async () => {
		const myDeviceGroupTopic =
			await this.contactsStore.devicesStore.myDeviceGroupTopic();
		const readHashes: Set<Hash> = new Set();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (
					op.body?.payload?.type === 'ReadMessages' &&
					op.body.payload.payload.chat_id === this.chatId
				) {
					for (const hash of op.body.payload.payload.message_hashes) {
						readHashes.add(hash);
					}
				}
			}
		}

		return readHashes;
	});

	unreadCount = reactive(async () => {
		const messages = await this.messages();
		const readHashes = await this.readMessageHashes();
		const myDeviceId = await this.contactsStore.myDeviceId();

		let count = 0;
		for (const [hash, message] of Object.entries(messages)) {
			// Only count messages from others (not our own)
			if (message.author !== myDeviceId && !readHashes.has(hash)) {
				count++;
			}
		}
		return count;
	});

	/** Sends the message and resolves with the operation id of the created
	 * message once it is confirmed in the local log. When `replyTo` is set,
	 * the message is sent as a reply to that message's latest known edit. */
	async sendMessage(input: {
		message: string;
		media: OutgoingMedia | null;
		replyTo?: Message | null;
	}): Promise<Hash> {
		// A reply targets the latest known edit of the message, like edits and
		// deletes do. Same staleness concern as `editMessage`: re-resolve from
		// the current map rather than the caller's snapshot.
		let reply: Hash | null = null;
		if (input.replyTo) {
			const fresh =
				(await this.messages())[input.replyTo.hash] ?? input.replyTo;
			reply = currentVersion(fresh).hash;
		}
		return this.client.sendMessage(
			this.chatId,
			input.message,
			input.media,
			reply,
		);
	}

	async markAsRead(messageHashes: Hash[]): Promise<void> {
		await this.client.markMessagesRead(this.chatId, messageHashes);
	}

	async sendReaction(reaction: ChatReaction) {
		await this.client.sendReaction(this.chatId, reaction);
	}

	async toggleReaction(message: Message, emoji: string) {
		if (!hasBody(message.content)) return;
		const myAgentId = await this.contactsStore.myAgentId();
		const newEmoji =
			message.content.reactions[myAgentId] === emoji ? null : emoji;
		await this.sendReaction({ target: message.hash, emoji: newEmoji });
	}

	async editMessage(message: Message, newText: string): Promise<Hash> {
		// Callers hold a snapshot captured when editing began; re-resolve so an
		// edit that arrived mid-compose is chained from, not forked off.
		const fresh = (await this.messages())[message.hash] ?? message;
		const current = currentVersion(fresh);
		return this.client.editMessage(this.chatId, current.hash, newText);
	}

	async deleteMessageForEveryone(message: Message): Promise<Hash> {
		// Same staleness concern as `editMessage`: the caller's snapshot may
		// predate an edit that arrived since.
		const fresh = (await this.messages())[message.hash] ?? message;
		const current = currentVersion(fresh);
		return this.client.deleteMessageForEveryone(this.chatId, current.hash);
	}

	async deleteMessageForMe(message: Message): Promise<Hash> {
		// The whole message is deleted regardless of which version is shown, so
		// target the original op; the backend tombstones its edit chain.
		return this.client.deleteMessageForMe(this.chatId, message.hash);
	}
}

// Apply each log item to the set of messages incrementally.
//
// `opsOrdered` is the interleaved list of all operations from all authors in the chat topic.
// It must be ordered so that any partial ordering constraints are upheld, i.e. items
// which reference prior items must appear after them.
//
// It is also assumed that all operations are valid.
function logsToMessages(
	opsOrdered: SimplifiedOperation<Payload>[],
	tombstones: Tombstones,
	deviceAgents: Record<DeviceId, AgentId>,
	deviceNames: Record<DeviceId, string>,
): Record<Hash, Message> {
	const messages: Record<Hash, Message | Bodyless> = {};
	// Map of EditMessage -> the target they reference
	const editTargets: Record<Hash, Hash> = {};

	for (const op of opsOrdered) {
		const author = op.header.verifying_key;
		const body = op.body;

		if (tombstones[op.hash] || !body) {
			// Put a bodyless placeholder for any deleted messages,
			// because we need something in place for the rest of the rendering to work.
			// Later we'll remove these and replace the original message with a proper
			// deleted-for-everyone placeholder if applicable.
			messages[op.hash] = {
				hash: op.hash,
				author,
				seqNum: op.header.seq_num,
				timestamp: op.header.timestamp,
			};
			continue;
		}
		if (body.type !== 'Chat') continue;

		if (body.payload.type === 'Message') {
			const quoteHash = body.payload.payload.reply
				? walkToRoot(body.payload.payload.reply, editTargets)
				: undefined;
			let replyQuote: MessageReply | undefined;
			if (quoteHash) {
				const replyTarget = messages[quoteHash];
				if (
					replyTarget &&
					isMessage(replyTarget) &&
					hasBody(replyTarget.content)
				) {
					// It's intended that the quoted text is the version of the message at the time of the reply.
					// This works as is only because messages are ordered by timestamp and we set this quote while
					// traversing the partial conversation. If replies were resolved during a second pass over the
					// conversation, we might pick up the current edited text instead, so be careful.
					replyQuote = {
						kind: 'content',
						author: replyTarget.author,
						authorName: deviceNames[replyTarget.author],
						text: replyTarget.content.message,
						media: mediaBundleToAttachment(replyTarget.content.media),
						scrollTarget: quoteHash,
					};
				} else if (tombstones[quoteHash] === 'DeletedForMe') {
					replyQuote = {
						kind: 'deleted-for-me',
					};
				} else if (tombstones[quoteHash] === 'DeletedForEveryone') {
					replyQuote = {
						kind: 'deleted',
						author: replyTarget?.author,
						authorName: replyTarget
							? deviceNames[replyTarget.author]
							: undefined,
						scrollTarget: replyTarget ? quoteHash : undefined,
					};
				} else {
					// Reply target was never received by this peer, or was invalid.
					console.warn('Reply target not covered:', quoteHash);
				}
			}
			messages[op.hash] = {
				hash: op.hash,
				content: {
					message: body.payload.payload.message,
					media: body.payload.payload.media,
					reactions: {},
					editHistory: [],
				},
				author,
				seqNum: op.header.seq_num,
				timestamp: op.header.timestamp,
				replyQuote: replyQuote,
			};
		} else if (body.payload.type === 'Reaction') {
			const { target, emoji } = body.payload.payload;
			const agent = deviceAgents[author];
			if (
				agent !== undefined &&
				messages[target] &&
				isMessage(messages[target]) &&
				hasBody(messages[target].content)
			) {
				if (emoji) {
					messages[target].content.reactions[agent] = emoji;
				} else {
					delete messages[target].content.reactions[agent];
				}
			}
		} else if (body.payload.type === 'EditMessage') {
			// TODO(after p2panda-spaces integration): this trusts every edit op in
			// the raw logs and enforces none of the backend's edit-validation
			// rules (`ValidChatOps::validate_edit` in
			// crates/dashchat-node/src/chat/edit.rs): author-only, at most one
			// edit per target resolved by (seq_num, hash), the 24h edit window,
			// and target-must-be-editable. A misbehaving peer's ops would
			// therefore render here. Once p2panda-spaces is integrated the
			// frontend should consume validated logs (or mirror
			// validate_edit) instead.
			const target = body.payload.payload.edit_hash;
			const root = walkToRoot(target, editTargets);
			if (
				messages[root] &&
				isMessage(messages[root]) &&
				hasBody(messages[root].content)
			) {
				editTargets[op.hash] = target;
				messages[root].content.message = body.payload.payload.message;
				messages[root].content.editHistory.push({
					hash: op.hash,
					text: body.payload.payload.message,
					timestamp: op.header.timestamp,
				});
			}
		} else if (body.payload.type === 'DeleteMessage') {
			const hashes = body.payload.payload.hashes;
			const deletes = hashes
				.map(hash => messages[hash])
				.filter(message => message !== undefined);
			if (deletes.length === 0) {
				// The original message was already tombstoned.
				continue;
			}
			const root = deletes.sort(
				(a, b) => a.timestamp - b.timestamp || a.hash.localeCompare(b.hash),
			)[0];
			const tombstoneReason = tombstones[root.hash];
			if (tombstoneReason) {
				if (tombstoneReason === 'DeletedForEveryone') {
					messages[root.hash] = placeholderFor(root);
				} else if (tombstoneReason === 'DeletedForMe') {
					// Just don't include it in the messages map
				}
			}
		}
	}

	// Filter out all bodyless placeholders
	const result: Record<Hash, Message> = {};
	for (const [hash, message] of Object.entries(messages)) {
		if (isMessage(message)) {
			result[hash] = message;
		} else {
			delete result[hash];
		}
	}

	return result;
}

function walkToRoot(hash: Hash, predecessors: Record<Hash, Hash>): Hash {
	let current = hash;
	while (predecessors[current]) {
		current = predecessors[current];
	}
	return current;
}

/** The placeholder a tombstoned operation renders as, built from its header
 * since its payload is gone. */
function placeholderFor(message: Bodyless): Message {
	return {
		hash: message.hash,
		content: 'deleted-for-everyone',
		author: message.author,
		seqNum: message.seqNum,
		timestamp: message.timestamp,
	};
}

function currentVersion(message: Message): MessageVersion {
	if (!hasBody(message.content)) {
		return { hash: message.hash, text: '', timestamp: message.timestamp };
	}
	const { editHistory } = message.content;
	if (editHistory.length > 0) {
		return editHistory[editHistory.length - 1];
	}
	return {
		hash: message.hash,
		text: message.content.message,
		timestamp: message.timestamp,
	};
}
