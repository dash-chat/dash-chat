import blakejs from 'blakejs';
import type {
	AgentId,
	AllMailboxesConnectionState,
	ChatId,
	ChatPayload,
	ColorSchemePreference,
	DeviceGroupPayload,
	DeviceId,
	GroupMember,
	GroupsArgs,
	Hash,
	MailboxConnectionHealth,
	MessageAcks,
	OperationSyncState,
	Payload,
	Profile,
	Settings,
	SimplifiedOperation,
	SystemEvent,
	Tombstones,
	TopicId,
} from 'dash-chat-stores';

import { DEMO_CONTACT_DEVICES, DEMO_IDS } from './demo-ids.ts';

type Operation = SimplifiedOperation<Payload>;

// Far future: a success or failure before the page's last resume would read
// as unproven rather than as the state it stands for.
const AFTER_ANY_RESUME = '9999-01-01T00:00:00Z';

export const MOCK_MAILBOX_UP: MailboxConnectionHealth = {
	status: 'Active',
	consecutive_errors: 0,
	next_poll_in_ms: 5000,
	last_success_at: AFTER_ANY_RESUME,
	last_error: null,
};

export const MOCK_MAILBOX_UNCONFIRMED: MailboxConnectionHealth = {
	...MOCK_MAILBOX_UP,
	last_success_at: null,
};

export const MOCK_MAILBOX_DOWN: MailboxConnectionHealth = {
	status: 'Stopped',
	consecutive_errors: 10,
	next_poll_in_ms: 5000,
	last_success_at: null,
	last_error: { at: AFTER_ANY_RESUME, message: 'unreachable' },
};

/** What the fake backend reports about the world outside the device. */
export interface World {
	mailboxes: AllMailboxesConnectionState;
	onlinePeers: AgentId[];
	operationsReachMailboxes: boolean;
	/** What other people's devices have acknowledged receiving, per author. */
	messageAcks: MessageAcks;
	/** Whether the local hub is kept off the LAN. */
	localHubBlocked: boolean;
}

// How Tauri puts a channel in the arguments of a command.
const CHANNEL_PREFIX = '__CHANNEL__:';

const CLOUD_MAILBOX = 'mock-cloud-mailbox';
const localMailbox = (index: number) => `mock-local-mailbox-${index}`;

/** Where what the backend sends without being asked goes: the page it is the
 * backend of. */
export interface Outbox {
	event(name: string, payload: unknown): void;
	/** A value of a stream the page subscribed to on `channel`. */
	stream(channel: number, index: number, value: unknown): void;
}

interface Stream {
	command: string;
	args: Record<string, unknown>;
	channel: number;
	sent: string[];
}

function hash(value: unknown): Hash {
	return blakejs.blake2bHex(JSON.stringify(value));
}

export function chatIdForDevices(deviceA: string, deviceB: string): string {
	return `mock-chat:${[deviceA, deviceB].sort().join(':')}`;
}

/** A stand-in for the app's backend, at the boundary the app reaches it
 * through: it answers the commands the app invokes, feeds the streams it
 * subscribes to, and emits the events it listens for. Everything is held in
 * memory. */
export class FakeBackend {
	private logs = new Map<TopicId, Map<DeviceId, Operation[]>>();
	private groupMembers = new Map<ChatId, GroupMember[]>();
	private streams = new Map<number, Stream>();
	private failures = new Map<string, string | undefined>();
	private answered = new Map<string, unknown>();
	private settings!: Settings;
	private colorSchemePreference!: ColorSchemePreference;
	world!: World;
	outbox: Outbox = { event: () => {}, stream: () => {} };

	constructor() {
		this.reset();
	}

	/** Back to a device nobody has set up. */
	reset(): void {
		this.logs.clear();
		this.groupMembers.clear();
		this.streams.clear();
		this.failures.clear();
		this.answered.clear();
		this.settings = {
			qr_color: null,
			local_mailbox_enabled: false,
			p2p_enabled: true,
			notifications_enabled: false,
			background_mode_enabled: false,
			developer_mode_enabled: false,
		};
		this.colorSchemePreference = 'system';
		this.world = {
			mailboxes: { cloudMailbox: MOCK_MAILBOX_UP, localMailboxes: [] },
			onlinePeers: [],
			operationsReachMailboxes: true,
			messageAcks: {},
			localHubBlocked: false,
		};
	}

	setWorld(changes: Partial<World>): void {
		Object.assign(this.world, changes);
		if (changes.messageAcks !== undefined) {
			for (const topic of this.logs.keys()) {
				this.emit({
					type: 'MessageAcks',
					payload: { topic, acks: changes.messageAcks },
				});
			}
		}
		this.feedStreams();
	}

	/** Have `command`, as the app spells it, fail from now on: with an error
	 * of `kind`, for the ones the UI tells apart. */
	fails(command: string, kind?: string): void {
		this.failures.set(command, kind);
	}

	/** Have `command` answer `answer` from now on. */
	answers(command: string, answer: unknown): void {
		this.answered.set(command, answer);
	}

	setSettings(changes: Partial<Settings>): void {
		Object.assign(this.settings, changes);
		this.outbox.event('settings://updated', { ...this.settings });
		this.feedStreams();
	}

	/** Write an operation to `topic` as `author`, resolving to its hash. */
	write(
		author: DeviceId,
		topic: TopicId,
		body: Payload | undefined,
		timestamp = Date.now(),
		auth?: GroupsArgs,
	): Hash {
		const log = this.logOf(topic, author);
		const previous = log.at(-1);
		const header = {
			backlink: previous?.hash,
			previous: [],
			verifying_key: author,
			seq_num: previous === undefined ? 0 : previous.header.seq_num + 1,
			timestamp,
			topic_id: topic,
			auth,
		};
		const operation: Operation = { body, hash: hash(header), header };
		log.push(operation);
		this.outbox.event('p2panda://new-operation', operation);
		this.emitTombstonesOf(operation);
		this.feedStreams();
		return operation.hash;
	}

	setGroupMembers(chatId: ChatId, members: GroupMember[]): void {
		const isNew = !this.groupMembers.has(chatId);
		this.groupMembers.set(chatId, members);
		this.emit({
			type: isNew ? 'GroupChatAdded' : 'GroupMembersChanged',
			payload: { chat_id: chatId },
		});
	}

	/** Answer `command`, the way the real backend would, or throw what it
	 * would fail with. */
	handle(command: string, args: Record<string, unknown> = {}): unknown {
		if (this.failures.has(command)) {
			const kind = this.failures.get(command);
			throw kind === undefined ? new Error('mock failure') : { kind };
		}
		if (this.answered.has(command)) return this.answered.get(command);
		if (typeof args.onEvent === 'string') {
			return this.subscribe(
				command,
				args,
				Number(args.onEvent.slice(CHANNEL_PREFIX.length)),
			);
		}
		const me = DEMO_IDS.MY_DEVICE_ID;
		switch (command) {
			case 'plugin:app|version':
				return 'preview';
			case 'plugin:resources|close':
				this.streams.delete(args.rid as number);
				return undefined;
			case 'plugin:system-theme|get_color_scheme_preference':
				return this.colorSchemePreference;
			case 'plugin:system-theme|set_color_scheme_preference':
				this.colorSchemePreference = args.scheme as ColorSchemePreference;
				this.outbox.event('system-theme://changed', args.scheme);
				return undefined;

			case 'get_log':
				return this.logOf(args.topicId as TopicId, args.author as DeviceId);
			case 'get_authors':
				return [...(this.logs.get(args.topicId as TopicId)?.keys() ?? [])];

			case 'my_agent_id':
				return DEMO_IDS.MY_AGENT_ID;
			case 'my_device_id':
				return me;
			case 'my_device_group_topic':
				return DEMO_IDS.DEVICE_GROUP_TOPIC;
			case 'active_inbox_topics':
				return [DEMO_IDS.INBOX_TOPIC];
			case 'agent_for_device':
				return this.agentFor(args.devicePubkey as DeviceId) ?? null;
			case 'direct_chat_id':
				return chatIdForDevices(me, args.peer as DeviceId);
			case 'get_profile':
				return null;
			case 'set_profile':
				this.write(me, DEMO_IDS.MY_AGENT_ID, {
					type: 'Announcements',
					payload: { type: 'SetProfile', payload: args.profile as Profile },
				});
				return undefined;
			case 'create_contact_code':
				return btoa(
					String.fromCharCode(...crypto.getRandomValues(new Uint8Array(45))),
				);
			case 'add_contact':
				return { kind: 'NewRequest', chatId: chatIdForDevices(me, me) };
			case 'accept_contact': {
				const agentId = args.agentId as AgentId;
				const peer = DEMO_CONTACT_DEVICES[agentId];
				if (peer === undefined) {
					throw new Error(`Unknown demo device for agent ${agentId}`);
				}
				this.writeToMyDeviceGroup({
					type: 'AddContact',
					payload: {
						agent_id: agentId,
						direct_chat_topic_id: chatIdForDevices(me, peer),
					},
				});
				return undefined;
			}
			case 'block_contact':
				this.writeToMyDeviceGroup({
					type: 'BlockAgent',
					payload: args.agentId as AgentId,
				});
				return undefined;
			case 'unblock_contact':
				this.writeToMyDeviceGroup({
					type: 'UnblockAgent',
					payload: args.agentId as AgentId,
				});
				return undefined;
			case 'report_contact':
				this.writeToMyDeviceGroup({
					type: 'ReportContact',
					payload: {
						agent_id: args.agentId as AgentId,
						device_ids: [],
						mailbox_ids: [],
					},
				});
				return undefined;

			case 'create_group': {
				const chatId = hash([Date.now(), Math.random()]);
				this.setGroupMembers(chatId, []);
				return chatId;
			}
			case 'get_group_chats':
				return [...this.groupMembers.keys()];
			case 'get_group_members':
				return this.groupMembers.get(args.chatId as ChatId) ?? [];

			case 'send_message':
				return this.writeToChat(args.chatId as ChatId, {
					type: 'Message',
					payload: {
						message: args.message as string,
						media: null,
						reply: (args.reply as Hash | null) ?? undefined,
					},
				});
			case 'send_reaction':
				this.writeToChat(args.chatId as ChatId, {
					type: 'Reaction',
					payload: args.content as Extract<
						ChatPayload,
						{ type: 'Reaction' }
					>['payload'],
				});
				return undefined;
			case 'edit_message':
				return this.writeToChat(args.chatId as ChatId, {
					type: 'EditMessage',
					payload: {
						message: args.message as string,
						edit_hash: args.editHash as Hash,
					},
				});
			case 'delete_message':
				return this.writeToChat(args.chatId as ChatId, {
					type: 'DeleteMessage',
					payload: {
						hashes: this.myEditChain(
							args.chatId as ChatId,
							args.targetHash as Hash,
						),
					},
				});
			case 'delete_message_for_me': {
				const chatId = args.chatId as ChatId;
				// The store passes the original message's hash, but whichever
				// version is pointed at, it is the original that is referenced.
				const messageHash = this.myEditChain(
					chatId,
					args.targetHash as Hash,
				).at(-1) as Hash;
				return this.writeToMyDeviceGroup({
					type: 'DeleteForMe',
					payload: { chat_id: chatId, message_hash: messageHash },
				});
			}
			case 'get_tombstones':
				return this.tombstonesOf(args.chatId as ChatId);
			case 'get_message_acks':
				return this.world.messageAcks;

			case 'get_settings':
				return { ...this.settings };
			case 'set_setting':
				this.setSettings({ [args.key as string]: args.value });
				return undefined;
			case 'set_local_mailbox_enabled':
				this.setSettings({ local_mailbox_enabled: args.enabled as boolean });
				return undefined;

			default:
				return undefined;
		}
	}

	private logOf(topic: TopicId, author: DeviceId): Operation[] {
		let authors = this.logs.get(topic);
		if (authors === undefined) {
			authors = new Map();
			this.logs.set(topic, authors);
		}
		let log = authors.get(author);
		if (log === undefined) {
			log = [];
			authors.set(author, log);
		}
		return log;
	}

	private operationsOf(topic: TopicId): Operation[] {
		return [...(this.logs.get(topic)?.values() ?? [])].flat();
	}

	private writeToChat(chatId: ChatId, payload: ChatPayload): Hash {
		return this.write(DEMO_IDS.MY_DEVICE_ID, chatId, { type: 'Chat', payload });
	}

	private writeToMyDeviceGroup(payload: DeviceGroupPayload): Hash {
		return this.write(DEMO_IDS.MY_DEVICE_ID, DEMO_IDS.DEVICE_GROUP_TOPIC, {
			type: 'DeviceGroupPayload',
			payload,
		});
	}

	private agentFor(device: DeviceId): AgentId | undefined {
		if (device === DEMO_IDS.MY_DEVICE_ID) return DEMO_IDS.MY_AGENT_ID;
		return Object.keys(DEMO_CONTACT_DEVICES).find(
			agent => DEMO_CONTACT_DEVICES[agent] === device,
		);
	}

	/** `target` and what it edits, back to the original message: a delete
	 * covers the whole chain. */
	private myEditChain(chatId: ChatId, target: Hash): Hash[] {
		const log = this.logOf(chatId, DEMO_IDS.MY_DEVICE_ID);
		const chain = [target];
		for (;;) {
			const body = log.find(operation => operation.hash === chain.at(-1))?.body;
			if (body?.type !== 'Chat' || body.payload.type !== 'EditMessage') {
				return chain;
			}
			chain.push(body.payload.payload.edit_hash);
		}
	}

	/** `root` and every edit that points at it, however indirectly. */
	private editsOf(chatId: ChatId, root: Hash): Hash[] {
		const closure = [root];
		for (const operation of this.operationsOf(chatId)) {
			const body = operation.body;
			if (
				body?.type === 'Chat' &&
				body.payload.type === 'EditMessage' &&
				closure.includes(body.payload.payload.edit_hash)
			) {
				closure.push(operation.hash);
			}
		}
		return closure;
	}

	private tombstonesOf(chatId: ChatId): Tombstones {
		const tombstones: Tombstones = {};
		for (const { body } of this.operationsOf(chatId)) {
			if (body?.type === 'Chat' && body.payload.type === 'DeleteMessage') {
				for (const deleted of body.payload.payload.hashes) {
					tombstones[deleted] = 'DeletedForEveryone';
				}
			}
		}
		for (const { body } of this.operationsOf(DEMO_IDS.DEVICE_GROUP_TOPIC)) {
			if (
				body?.type === 'DeviceGroupPayload' &&
				body.payload.type === 'DeleteForMe' &&
				body.payload.payload.chat_id === chatId
			) {
				for (const deleted of this.editsOf(
					chatId,
					body.payload.payload.message_hash,
				)) {
					tombstones[deleted] = 'DeletedForMe';
				}
			}
		}
		return tombstones;
	}

	private emitTombstonesOf({ body, header }: Operation): void {
		if (body?.type === 'Chat' && body.payload.type === 'DeleteMessage') {
			this.emit({
				type: 'Tombstones',
				payload: {
					topic: header.topic_id,
					hashes: body.payload.payload.hashes,
					reason: 'DeletedForEveryone',
				},
			});
		}
		if (
			body?.type === 'DeviceGroupPayload' &&
			body.payload.type === 'DeleteForMe'
		) {
			const { chat_id, message_hash } = body.payload.payload;
			this.emit({
				type: 'Tombstones',
				payload: {
					topic: chat_id,
					hashes: this.editsOf(chat_id, message_hash),
					reason: 'DeletedForMe',
				},
			});
		}
	}

	private emit(event: SystemEvent): void {
		this.outbox.event('dashchat://system-event', event);
	}

	private subscribe(
		command: string,
		args: Record<string, unknown>,
		channel: number,
	): number {
		const rid = Math.max(0, ...this.streams.keys()) + 1;
		this.streams.set(rid, { command, args, channel, sent: [] });
		this.feedStreams();
		return rid;
	}

	/** Send each stream its value, where it is not the one it last got. */
	private feedStreams(): void {
		for (const stream of this.streams.values()) {
			const value = JSON.stringify(this.streamed(stream.command, stream.args));
			if (stream.sent.at(-1) === value) continue;
			this.outbox.stream(stream.channel, stream.sent.length, JSON.parse(value));
			stream.sent.push(value);
		}
	}

	private streamed(command: string, args: Record<string, unknown>): unknown {
		const { cloudMailbox, localMailboxes } = this.world.mailboxes;
		const mailboxIds = [
			...(cloudMailbox === null ? [] : [CLOUD_MAILBOX]),
			...localMailboxes.map((_, index) => localMailbox(index)),
		];
		switch (command) {
			case 'mailbox_subscribe_active_ids':
			case 'mailbox_subscribe_all_ids':
				return mailboxIds;
			case 'mailbox_subscribe_cloud_id':
				return cloudMailbox === null ? null : CLOUD_MAILBOX;
			case 'mailbox_subscribe_connection_state':
				return args.mailboxId === CLOUD_MAILBOX
					? cloudMailbox
					: localMailboxes.find(
							(_, index) => localMailbox(index) === args.mailboxId,
						);
			case 'mailbox_subscribe_sync_state':
				return args.mailboxId === CLOUD_MAILBOX &&
					this.world.operationsReachMailboxes
					? this.everythingWritten()
					: {};
			case 'subscribe_to_online_peers':
				return this.world.onlinePeers.map(agent => DEMO_CONTACT_DEVICES[agent]);
			case 'subscribe_to_lan_multicast_blocked':
				return this.world.localHubBlocked;
			default:
				return null;
		}
	}

	private everythingWritten(): OperationSyncState {
		const written: OperationSyncState = {};
		for (const [topic, authors] of this.logs) {
			written[topic] = {};
			for (const [author, log] of authors) {
				if (log.length > 0) written[topic][author] = log.length - 1;
			}
		}
		return written;
	}
}

/** The one fake backend there is: of the browser preview in the page, and of
 * each scene in the test that plays it. */
export const backend = new FakeBackend();
