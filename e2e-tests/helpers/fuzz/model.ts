/**
 * The expected-state model of a fuzz run. Every effect a move produces is an
 * op on a topic; every agent and every hub is a holder that knows a set of
 * ops. Moves record their own ops on the acting agent, `propagate` spreads
 * knowledge the way the app does — per topic, among the holders of a LAN
 * that subscribe to it — and `view` derives what an agent's screen must show
 * from what it knows. It holds only names and expected state, never browser
 * handles: `check(model)` must stay pure.
 */

export type MessageKind = 'text' | 'photo' | 'file' | 'voice';
export type OpId = string;
type Topic = string;

export interface ExpectedChat {
	kind: 'direct' | 'group';
	/** Group name; '' for direct chats, which are opened by peer name. */
	name: string;
	/** Group creator; '' for direct chats. */
	creator: string;
	members: string[];
}

export interface ExpectedMessage {
	/** Unique greppable id, always contained in the rendered text (or media
	 * name), so it can be waited for without colliding with any other
	 * message of the run. */
	label: string;
	kind: MessageKind;
	sender: string;
	chat: ExpectedChat;
	/** Label of the message this one replies to. */
	replyTo?: string;
	/** A replied-to message is never deleted: its text lives on in the reply
	 * quote, which would make "label gone from the chat" unverifiable. */
	hasReply: boolean;
	edits: number;
}

/** One message as an agent that knows a given set of ops must see it. */
export interface MessageView {
	label: string;
	kind: MessageKind;
	text: string;
	deleted: boolean;
	/** Media only: whether the bytes have arrived. */
	loaded: boolean;
	/** Reactor name → emoji. */
	reactions: Map<string, string>;
	replyTo?: string;
}

export interface ChatView {
	/** Direct chats only: the peer's profile has not arrived yet. */
	pending: boolean;
	messages: MessageView[];
}

export interface ExpectedHub {
	name: string;
	/** The LAN it is on, or null while it is on none. */
	network: string | null;
	/** Whether its process is up: only a running hub is shown and relays. */
	running: boolean;
}

/** A LAN available to a run, by its real network name. */
export interface ExpectedNetwork {
	name: string;
}

export interface InteractionTarget {
	chat: ExpectedChat;
	message: ExpectedMessage;
	view: MessageView;
}

type Op =
	| { id: OpId; topic: Topic; kind: 'profile'; agent: string }
	| { id: OpId; topic: Topic; kind: 'group'; chat: ExpectedChat }
	| { id: OpId; topic: Topic; kind: 'message'; message: ExpectedMessage }
	| { id: OpId; topic: Topic; kind: 'bytes'; message: ExpectedMessage }
	| {
			id: OpId;
			topic: Topic;
			kind: 'edit';
			message: ExpectedMessage;
			text: string;
	  }
	| { id: OpId; topic: Topic; kind: 'delete'; message: ExpectedMessage }
	| {
			id: OpId;
			topic: Topic;
			kind: 'reaction';
			message: ExpectedMessage;
			reactor: string;
			emoji: string;
	  };

/** How many of a chat's latest messages moves may interact with — recent
 * enough to still be rendered near the bottom without scrolling. */
const RECENT_WINDOW = 8;

function profileOp(agent: string): OpId {
	return `profile:${agent}`;
}

function groupOp(chat: ExpectedChat, member: string): OpId {
	return `group:${chat.name}>${member}`;
}

function chatTopic(chat: ExpectedChat): Topic {
	if (chat.kind === 'group') return `chat:g:${chat.name}`;
	return `chat:d:${[...chat.members].sort().join('+')}`;
}

export class ExpectedModel {
	readonly agents: { name: string; mobile: boolean }[];
	readonly chats: ExpectedChat[] = [];
	/** The LANs the run may use, up for its whole length. None means every
	 * agent shares its usual LAN, and keeps every hub and network move out
	 * of the run. */
	readonly networks: ExpectedNetwork[];
	readonly hubs: ExpectedHub[] = [];
	private readonly messages = new Map<string, ExpectedMessage>();
	private readonly ops: Op[] = [];
	private readonly opsByTopic = new Map<Topic, Op[]>();
	private readonly chatByTopic = new Map<Topic, ExpectedChat>();
	/** Holder name → the ops it holds. */
	private readonly knowledge = new Map<string, Set<OpId>>();
	/** One-sided adds already performed, as 'from>to'. A pair are contacts —
	 * and their direct chat enters the model — once both directions exist. */
	private readonly added = new Set<string>();
	/** Agents currently backgrounded: off the network, their UI undriveable. */
	private readonly backgrounded = new Set<string>();
	/** Agent name → the LAN it is on; absent while it is on none. */
	private readonly network = new Map<string, string>();
	private messageCounter = 0;
	private groupCounter = 0;
	private hubCounter = 0;

	constructor(
		agents: { name: string; mobile: boolean }[],
		networks: string[] = [],
	) {
		this.agents = agents;
		this.networks = networks.map(name => ({ name }));
		for (const { name } of agents) {
			this.knowledge.set(name, new Set());
			this.record(name, {
				id: profileOp(name),
				topic: `announce:${name}`,
				kind: 'profile',
				agent: name,
			});
		}
	}

	names(): string[] {
		return this.agents.map(a => a.name);
	}

	/** Agents whose UI can be driven right now. */
	activeNames(): string[] {
		return this.names().filter(n => !this.backgrounded.has(n));
	}

	activeMobileNames(): string[] {
		return this.agents
			.filter(a => a.mobile && !this.backgrounded.has(a.name))
			.map(a => a.name);
	}

	backgroundedNames(): string[] {
		return [...this.backgrounded];
	}

	isBackgrounded(name: string): boolean {
		return this.backgrounded.has(name);
	}

	background(name: string): void {
		this.backgrounded.add(name);
	}

	foreground(name: string): void {
		this.backgrounded.delete(name);
	}

	/** The LAN `name` is on, or null while it is on none. */
	networkOf(name: string): string | null {
		return this.network.get(name) ?? null;
	}

	/** Networks up that `name` is not on. */
	otherNetworks(name: string): string[] {
		return this.networkNames().filter(n => n !== this.networkOf(name));
	}

	agentJoin(name: string, network: string): void {
		this.network.set(name, network);
	}

	agentLeave(name: string): void {
		this.network.delete(name);
	}

	/** Agents whose UI can be driven right now, on `network`. */
	activeNamesOn(network: string): string[] {
		return this.activeNames().filter(n => this.networkOf(n) === network);
	}

	networkNames(): string[] {
		return this.networks.map(n => n.name);
	}

	hasNetworks(): boolean {
		return this.networks.length > 0;
	}

	/** Zero-padded so `hub-01` can never substring-match `hub-010`. */
	createHub(): ExpectedHub {
		const hub: ExpectedHub = {
			name: `hub-${String(++this.hubCounter).padStart(2, '0')}`,
			network: null,
			running: false,
		};
		this.hubs.push(hub);
		this.knowledge.set(hub.name, new Set());
		return hub;
	}

	hub(name: string): ExpectedHub {
		const found = this.hubs.find(h => h.name === name);
		if (found === undefined) throw new Error(`no hub named ${name}`);
		return found;
	}

	hubJoin(name: string, network: string): void {
		this.hub(name).network = network;
	}

	hubLeave(name: string): void {
		this.hub(name).network = null;
	}

	startHub(name: string): void {
		this.hub(name).running = true;
	}

	stopHub(name: string): void {
		this.hub(name).running = false;
	}

	/** The running hubs on `network`. */
	private hubsOn(network: string): ExpectedHub[] {
		return this.hubs.filter(h => h.running && h.network === network);
	}

	/** Hubs `name`'s app must show as connected: the running ones on its
	 * network, none while it is on no network. */
	expectedHubs(name: string): number {
		const network = this.networkOf(name);
		if (network === null) return 0;
		return this.hubsOn(network).length;
	}

	areContacts(a: string, b: string): boolean {
		return this.added.has(`${a}>${b}`) && this.added.has(`${b}>${a}`);
	}

	/** The direct chat between `a` and `b`, which exists once they are contacts. */
	directChat(a: string, b: string): ExpectedChat {
		const chat = this.chats.find(
			c =>
				c.kind === 'direct' && c.members.includes(a) && c.members.includes(b),
		);
		if (chat === undefined) throw new Error(`${a} and ${b} are not contacts`);
		return chat;
	}

	/** Record that `from` entered `to`'s add-contact link. Once the reverse
	 * direction exists too, the pair's direct chat is added to the model. */
	recordAdded(from: string, to: string): void {
		this.added.add(`${from}>${to}`);
		if (!this.added.has(`${to}>${from}`)) return;
		this.addChat({
			kind: 'direct',
			name: '',
			creator: '',
			members: [from, to],
		});
	}

	/** Peers whose add-contact link `name` has not entered yet. */
	notYetAdded(name: string): string[] {
		return this.names().filter(
			other => other !== name && !this.added.has(`${name}>${other}`),
		);
	}

	/** The contacts `name` can act on: a mutual add whose profile has also
	 * reached `name`. Adding is a local act, but the peer's profile arrives as
	 * a separate op — until it does the app has only a bare key, so it shows no
	 * contact row and the group-member picker offers none. */
	contactsOf(name: string): string[] {
		return this.names().filter(
			other =>
				other !== name &&
				this.areContacts(name, other) &&
				this.knows(name).has(profileOp(other)),
		);
	}

	/** Chats `name` can open: the direct chats of its contacts, and the
	 * groups it created or has learnt about. */
	chatsFor(name: string): ExpectedChat[] {
		return this.chats.filter(
			c =>
				c.members.includes(name) &&
				(c.kind === 'direct' || this.hasGroup(name, c)),
		);
	}

	/** Chats `name` can send in: the ones it can open that are not pending. */
	sendableChatsFor(name: string): ExpectedChat[] {
		return this.chatsFor(name).filter(c => !this.view(name, c).pending);
	}

	/** Create a group: the creator knows it at once; each invited member
	 * learns of it through its inbox. */
	addGroup(creator: string, members: string[], name: string): ExpectedChat {
		const chat: ExpectedChat = {
			kind: 'group',
			name,
			creator,
			members: [creator, ...members],
		};
		this.addChat(chat);
		for (const member of members) {
			this.record(creator, {
				id: groupOp(chat, member),
				topic: `inbox:${member}`,
				kind: 'group',
				chat,
			});
		}
		return chat;
	}

	/** Zero-padded so `group-001` can never substring-match `group-0010`. */
	nextGroupName(): string {
		return `group-${String(++this.groupCounter).padStart(3, '0')}`;
	}

	/** Zero-padded so `sm-alice-0001` can never substring-match a later label. */
	nextLabel(sender: string): string {
		const n = String(++this.messageCounter).padStart(4, '0');
		return `sm-${sender.toLowerCase()}-${n}`;
	}

	/** A whole number of seconds no other voice note of the run has, so its
	 * rendered duration identifies it. */
	nextVoiceSeconds(): number {
		return ++this.messageCounter;
	}

	/** Record a message the sender just sent; media come with their bytes. */
	addMessage(
		chat: ExpectedChat,
		sender: string,
		kind: MessageKind,
		label: string,
		replyTo?: string,
	): ExpectedMessage {
		const message: ExpectedMessage = {
			label,
			kind,
			sender,
			chat,
			replyTo,
			hasReply: false,
			edits: 0,
		};
		this.messages.set(label, message);
		if (replyTo !== undefined) this.message(replyTo).hasReply = true;
		const topic = chatTopic(chat);
		this.record(sender, {
			id: `message:${label}`,
			topic,
			kind: 'message',
			message,
		});
		if (kind !== 'text') {
			this.record(sender, {
				id: `bytes:${label}`,
				topic,
				kind: 'bytes',
				message,
			});
		}
		return message;
	}

	/** Record an edit by the sender; returns the new text. */
	recordEdit(message: ExpectedMessage): string {
		const n = ++message.edits;
		const text = `${message.label} v${n}`;
		this.record(message.sender, {
			id: `edit:${message.label}:${n}`,
			topic: chatTopic(message.chat),
			kind: 'edit',
			message,
			text,
		});
		return text;
	}

	recordDelete(message: ExpectedMessage): void {
		this.record(message.sender, {
			id: `delete:${message.label}`,
			topic: chatTopic(message.chat),
			kind: 'delete',
			message,
		});
	}

	recordReaction(
		message: ExpectedMessage,
		reactor: string,
		emoji: string,
	): void {
		this.record(reactor, {
			id: `reaction:${message.label}:${reactor}:${emoji}`,
			topic: chatTopic(message.chat),
			kind: 'reaction',
			message,
			reactor,
			emoji,
		});
	}

	/** How `chat` appears in `viewer`'s chat list. */
	chatListName(chat: ExpectedChat, viewer: string): string {
		if (chat.kind === 'group') return chat.name;
		const peer = chat.members.find(m => m !== viewer);
		if (peer === undefined) {
			throw new Error(`direct chat has no peer for ${viewer}`);
		}
		return peer;
	}

	/** Recent live text messages `name` sees and may interact with (react,
	 * reply, and — with `ownOnly` — edit or delete), across its chats. */
	interactionTargets(name: string, ownOnly = false): InteractionTarget[] {
		return this.chatsFor(name).flatMap(chat =>
			this.view(name, chat)
				.messages.filter(v => v.kind === 'text' && !v.deleted)
				.slice(-RECENT_WINDOW)
				.map(view => ({ chat, message: this.message(view.label), view }))
				.filter(t => !ownOnly || t.message.sender === name),
		);
	}

	/** The ops `holder` — an agent or a hub — holds. */
	knows(holder: string): ReadonlySet<OpId> {
		const known = this.knowledge.get(holder);
		if (known === undefined) throw new Error(`no holder named ${holder}`);
		return known;
	}

	/** What `agent`'s screen must show for `chat`, from the ops it knows. */
	view(agent: string, chat: ExpectedChat): ChatView {
		const known = this.knows(agent);
		const views = new Map<string, MessageView>();
		for (const op of this.opsByTopic.get(chatTopic(chat)) ?? []) {
			if (!known.has(op.id)) continue;
			this.fold(views, op);
		}
		const peer = chat.kind === 'direct' ? this.chatListName(chat, agent) : null;
		return {
			pending: peer !== null && !known.has(profileOp(peer)),
			messages: [...views.values()],
		};
	}

	/**
	 * Spread knowledge as the app does: within each LAN, for each topic, the
	 * holders subscribed to it end up with the union of their ops of it. A hub
	 * subscribes to everything; a backgrounded agent is on no LAN. Iterated
	 * until nothing changes, since learning a group subscribes to its chat.
	 * Returns, per agent that learnt something, the chats whose view changed.
	 */
	propagate(): Map<string, ExpectedChat[]> {
		return this.spread(() => this.components());
	}

	/** Spread knowledge as if every agent shared one LAN: where they are
	 * after being together on their usual network, as during preparation. */
	propagateShared(): Map<string, ExpectedChat[]> {
		return this.spread(() => [this.activeNames()]);
	}

	private spread(components: () => string[][]): Map<string, ExpectedChat[]> {
		const before = new Map(
			this.names().map(name => [name, new Set(this.knows(name))]),
		);
		let changed = true;
		while (changed) {
			changed = false;
			for (const component of components()) {
				if (this.unionComponent(component)) changed = true;
			}
		}
		const growth = new Map<string, ExpectedChat[]>();
		for (const name of this.names()) {
			const chats = this.chatsGained(name, before.get(name) ?? new Set());
			if (chats.length > 0) growth.set(name, chats);
		}
		return growth;
	}

	private message(label: string): ExpectedMessage {
		const found = this.messages.get(label);
		if (found === undefined) throw new Error(`no message labelled ${label}`);
		return found;
	}

	private hasGroup(name: string, chat: ExpectedChat): boolean {
		return chat.creator === name || this.knows(name).has(groupOp(chat, name));
	}

	private addChat(chat: ExpectedChat): void {
		this.chats.push(chat);
		this.chatByTopic.set(chatTopic(chat), chat);
	}

	private record(holder: string, op: Op): void {
		this.ops.push(op);
		const ops = this.opsByTopic.get(op.topic);
		if (ops === undefined) this.opsByTopic.set(op.topic, [op]);
		else ops.push(op);
		this.knowledge.get(holder)?.add(op.id);
	}

	private fold(views: Map<string, MessageView>, op: Op): void {
		if (op.kind === 'profile' || op.kind === 'group') return;
		const { label, kind, replyTo } = op.message;
		if (op.kind === 'message') {
			views.set(label, {
				label,
				kind,
				text: label,
				deleted: false,
				loaded: false,
				reactions: new Map(),
				replyTo,
			});
			return;
		}
		const view = views.get(label);
		if (view === undefined) return;
		if (op.kind === 'bytes') view.loaded = true;
		else if (op.kind === 'edit') view.text = op.text;
		else if (op.kind === 'delete') view.deleted = true;
		else view.reactions.set(op.reactor, op.emoji);
	}

	/** Topics `holder` syncs, or null for a hub, which syncs them all. */
	private subscriptions(holder: string): Set<Topic> | null {
		if (this.hubs.some(h => h.name === holder)) return null;
		const topics = new Set<Topic>([`announce:${holder}`, `inbox:${holder}`]);
		for (const other of this.names()) {
			if (!this.added.has(`${holder}>${other}`)) continue;
			topics.add(`announce:${other}`);
			topics.add(`inbox:${other}`);
		}
		for (const chat of this.chatsFor(holder)) topics.add(chatTopic(chat));
		return topics;
	}

	/** The holders that can sync with each other right now, grouped. */
	private components(): string[][] {
		if (!this.hasNetworks()) return [this.activeNames()];
		return this.networks.map(n => [
			...this.hubsOn(n.name).map(h => h.name),
			...this.activeNamesOn(n.name),
		]);
	}

	/** One round of per-topic union within `component`; whether anything
	 * changed. */
	private unionComponent(component: string[]): boolean {
		const subscribers = new Map<Topic, string[]>();
		const hubs: string[] = [];
		for (const holder of component) {
			const topics = this.subscriptions(holder);
			if (topics === null) {
				hubs.push(holder);
				continue;
			}
			for (const topic of topics) {
				subscribers.set(topic, [...(subscribers.get(topic) ?? []), holder]);
			}
		}
		let changed = false;
		for (const [topic, ops] of this.opsByTopic) {
			const holders = [...(subscribers.get(topic) ?? []), ...hubs];
			if (holders.length < 2) continue;
			if (this.unionTopic(holders, ops)) changed = true;
		}
		return changed;
	}

	private unionTopic(holders: string[], ops: Op[]): boolean {
		const union = ops.filter(op => holders.some(h => this.knows(h).has(op.id)));
		let changed = false;
		for (const holder of holders) {
			const known = this.knowledge.get(holder);
			if (known === undefined) continue;
			for (const op of union) {
				if (known.has(op.id)) continue;
				known.add(op.id);
				changed = true;
			}
		}
		return changed;
	}

	/** The chats whose view for `name` changed since it knew `before`. */
	private chatsGained(name: string, before: ReadonlySet<OpId>): ExpectedChat[] {
		const gained = new Set<ExpectedChat>();
		for (const op of this.ops) {
			if (before.has(op.id) || !this.knows(name).has(op.id)) continue;
			const chat = this.chatOf(name, op);
			if (chat !== null) gained.add(chat);
		}
		return this.chats.filter(c => gained.has(c));
	}

	private chatOf(name: string, op: Op): ExpectedChat | null {
		if (op.kind === 'group') return op.chat;
		if (op.kind === 'profile') {
			return (
				this.chats.find(
					c =>
						c.kind === 'direct' &&
						c.members.includes(name) &&
						c.members.includes(op.agent),
				) ?? null
			);
		}
		return this.chatByTopic.get(op.topic) ?? null;
	}
}

/** The model for `real`, before anything has happened: no chats, no
 * contacts, no hub anywhere, everyone foregrounded and off the air. */
export function newModel(real: {
	agents: { agent: { platform: string }; name: string }[];
	networks: { ssid: string }[];
}): ExpectedModel {
	return new ExpectedModel(
		real.agents.map(({ agent, name }) => ({
			name,
			mobile: agent.platform !== 'desktop',
		})),
		real.networks.map(n => n.ssid),
	);
}
