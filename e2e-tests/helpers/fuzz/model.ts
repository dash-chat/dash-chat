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
	/** What a group is, whatever it is called: its name can be changed, and
	 * two viewers can be showing different ones. '' for direct chats, which
	 * are the pair that is in them. */
	id: string;
	/** The name a group was created with, which is what a viewer that has
	 * learnt no later one shows. '' for direct chats, which are opened by
	 * peer name. */
	name: string;
	/** Group creator; '' for direct chats. */
	creator: string;
	/** A direct chat's pair, which never changes, or the members a group was
	 * created with. Joining and leaving are ops like any other, so who is in
	 * a group now is what each device has heard: ask `membersFor`. */
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
	/** Photos only: how many the message carries. */
	photos?: number;
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
	/** Direct chats only: the viewer has blocked the peer, which turns the
	 * chat read-only. */
	blocked: boolean;
	/** Groups only: the viewer has left or been removed. `chatsFor` drops such
	 * a group, so no row is expected for it — this is only ever read of the
	 * chat an agent is still sitting in when the departure reaches it, where
	 * the app replaces the composer with why there is none. */
	departed: boolean;
	messages: MessageView[];
}

/** One notification a device has posted and not yet cleared. */
export interface NotificationView {
	/** The route it opens, and whose opening clears it. */
	route: string;
	/** The chat a tap must land on, or null for one whose chat the agent
	 * cannot open yet — a contact request from someone it has not added back. */
	opens: ExpectedChat | null;
	/** Strings the device must be showing for it. */
	shows: string[];
	/** Strings of which the device must be showing at least one: a chat's
	 * entry carries a message of the ones that arrived unread, and which it
	 * is depends on the order a healed link delivered them in. Empty when
	 * there is nothing to choose between. */
	oneOf: string[];
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
	/** The host's own LAN: every running hub is on it, whatever LAN the
	 * host's card is on, since the host reaches it by wire too. */
	home: boolean;
}

export interface InteractionTarget {
	chat: ExpectedChat;
	message: ExpectedMessage;
	view: MessageView;
}

type Op =
	| { id: OpId; topic: Topic; kind: 'profile'; agent: string; name: string }
	| { id: OpId; topic: Topic; kind: 'accept'; from: string; to: string }
	| {
			id: OpId;
			topic: Topic;
			kind: 'request';
			from: string;
			to: string;
			/** What `from` called itself when it sent this, which the request
			 * carries to a device that has no profile of it yet. */
			name: string;
	  }
	| {
			id: OpId;
			topic: Topic;
			kind: 'group';
			chat: ExpectedChat;
			member: string;
			by: string;
	  }
	| {
			id: OpId;
			topic: Topic;
			kind: 'groupRemoved';
			chat: ExpectedChat;
			member: string;
			by: string;
	  }
	| {
			id: OpId;
			topic: Topic;
			kind: 'groupInfo';
			chat: ExpectedChat;
			name: string;
			description: string;
	  }
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

/** One agent as the model knows it. `p2p: false` is an agent run without
 * peer-to-peer connectivity: on no LAN component, it reaches nothing but the
 * cloud. `notifications` is null for an agent whose device the run does not
 * read. */
export interface ExpectedAgent {
	name: string;
	mobile: boolean;
	p2p?: boolean;
	notifications: NotificationTexts | null;
}

/** A join or a departure: what a device folds into who is in a group. */
type MembershipOp = Extract<Op, { kind: 'group' | 'groupRemoved' }>;

/** How many of a chat's latest messages moves may interact with — recent
 * enough to still be rendered near the bottom without scrolling. */
const RECENT_WINDOW = 8;

/** The op kinds a viewer's chat filters by authorship time when it had their
 * author blocked. A group's info is not among them: it stays visible. */
const CHAT_KINDS: Op['kind'][] = [
	'message',
	'bytes',
	'edit',
	'delete',
	'reaction',
];

function requestOp(from: string, to: string): OpId {
	return `request:${from}>${to}`;
}

/** What `accepter` publishes when a request from someone it had already
 *  entered the link of arrives: the node marks the pair a contact on its own
 *  device group and replies, which is what lets the requester do the same. */
function acceptOp(accepter: string, requester: string): OpId {
	return `accept:${accepter}>${requester}`;
}

function profileOp(agent: string, version: number): OpId {
	return `profile:${agent}:${version}`;
}

/** The strings a message notification's body is sure to carry: a text
 * message's text, and a file's name, which the placeholder standing in for it
 * spells out. A photo or a voice note gets a placeholder that names a kind,
 * not which one it is. */
function bodyOf(
	message: ExpectedMessage,
	texts: NotificationTexts | null,
): string[] {
	if (message.kind === 'text' || message.kind === 'file')
		return [message.label];
	if (texts === null) return [];
	if (message.kind === 'voice') return [texts.voice];
	return [texts.photos[message.photos ?? 1]];
}

/** The number a name ends in, so adopting `group-003` stops a run minting
 * it again. */
function suffixNumber(name: string): number {
	const match = /(\d+)$/.exec(name);
	return match === null ? 0 : Number(match[1]);
}

/** The seconds a rendered voice duration stands for: it is the label such a
 * message goes by, and [`nextVoiceSeconds`] mints from the same counter. */
function durationSeconds(label: string): number {
	const match = /^(\d+):(\d{2})$/.exec(label);
	return match === null ? 0 : Number(match[1]) * 60 + Number(match[2]);
}

/** Who published `op`. What blocking turns on: everything a blocked author
 * writes is thrown away as it arrives. */
function authorOf(op: Op): string {
	if (op.kind === 'profile') return op.agent;
	if (op.kind === 'request' || op.kind === 'accept') return op.from;
	if (op.kind === 'group' || op.kind === 'groupRemoved') return op.by;
	if (op.kind === 'groupInfo') return op.chat.creator;
	if (op.kind === 'reaction') return op.reactor;
	return op.message.sender;
}

function chatTopic(chat: ExpectedChat): Topic {
	if (chat.kind === 'group') return `chat:g:${chat.id}`;
	return directTopic(...(chat.members as [string, string]));
}

/** The direct chat's topic, which a contact request already routes to before
 * the pair are contacts and the chat itself exists. */
function directTopic(a: string, b: string): Topic {
	return `chat:d:${[a, b].sort().join('+')}`;
}

/** The cloud mailbox as a holder: it subscribes to everything, like a hub,
 * and every foregrounded agent reaches it while its link is usable. */
const CLOUD = 'cloud';

/** The photo counts a send may carry. The wording a caption-less photo
 * message is announced with depends on how many it holds, so the run reads a
 * translation for each of these. */
export const PHOTO_COUNTS = [1, 2, 3];

/** What an app writes into a notification it cannot fully name, read off the
 * device so the model compares against the locale it is running in. */
export interface NotificationTexts {
	/** "You have a new message": what a push it cannot fetch produces. */
	generic: string;
	/** The title before the sender's profile has arrived. */
	unnamedSender: string;
	/** The body a caption-less photo message gets by how many photos it
	 * carries, less its emoji — "Photo" for one, "3 photos" for three. */
	photos: Record<number, string>;
	/** The body a voice note gets, less its emoji. */
	voice: string;
}

export class ExpectedModel {
	/** `p2p: false` is an agent run without peer-to-peer connectivity: on no
	 * LAN component, it reaches nothing but the cloud. */
	readonly agents: ExpectedAgent[];
	readonly chats: ExpectedChat[] = [];
	/** The LANs the run may use, up for its whole length. None means every
	 * agent shares its usual LAN, and keeps every hub and network move out
	 * of the run. */
	readonly networks: ExpectedNetwork[];
	readonly hubs: ExpectedHub[] = [];
	/** The cloud mailbox. Every run has one — the harness spawns it and the
	 *  phones reach it — so what varies is only whether agents can reach it
	 *  right now. */
	readonly cloud: { usable: boolean };
	/** Whether the run can degrade the link to it, which keeps the cloud
	 *  moves out of a run whose mailbox the proxy does not front. */
	readonly cloudDegradable: boolean;
	/** Whether the run delivers pushes, which is what reaches a phone whose
	 * app has been killed. */
	private readonly push: boolean;
	private readonly messages = new Map<string, ExpectedMessage>();
	private readonly ops: Op[] = [];
	private readonly opsByTopic = new Map<Topic, Op[]>();
	private readonly chatByTopic = new Map<Topic, ExpectedChat>();
	/** Holder name → the ops it holds. */
	private readonly knowledge = new Map<string, Set<OpId>>();
	/** Agent name → the notifications its device is showing, by the id the OS
	 * keeps them under: a chat's messages share one, so a later message
	 * replaces the entry, while each request and each group invite gets its
	 * own. */
	private readonly posted = new Map<string, Map<string, NotificationView>>();
	/** Agent name → chat route → the messages that landed there while its app
	 * was not showing that chat: what the row's unread badge counts. Opening
	 * the chat reads every one of them, as entering it does on the device. */
	private readonly unread = new Map<string, Map<Topic, Set<OpId>>>();
	/** Agent name → the route it has open and, when the model has one, the
	 * chat there; absent while it is on the chat list. Kept by route because
	 * an agent can be looking at a chat the model cannot name yet — the one a
	 * contact request opens. An app resumes onto what it was showing, so this
	 * outlives backgrounding. */
	private readonly viewing = new Map<
		string,
		{ route: Topic; chat: ExpectedChat | null }
	>();
	/** One-sided adds already performed, as 'from>to'. A pair are contacts —
	 * and their direct chat enters the model — once both directions exist. */
	private readonly added = new Set<string>();
	/** Agents currently backgrounded: their app is alive and syncing, their
	 * UI undriveable. */
	private readonly backgrounded = new Set<string>();
	/** Agents whose app is stopped: only a push wakes them, and their UI is
	 * undriveable. */
	private readonly stopped = new Set<string>();
	/** Agent name → the profiles it has published, oldest first: what a
	 * device calls them is the last of these it has heard. */
	private readonly profiles = new Map<string, Op[]>();
	/** The name each contact request carries, by the op that carries it. */
	private readonly requestNames = new Map<OpId, string>();
	/** Agent name → the group preparation made for it to read its connection
	 * chip in. */
	private readonly chipChats = new Map<string, ExpectedChat>();
	/** Group id → the joins and departures of that group, in the order they
	 * happened: what each device has heard of them is its membership. */
	private readonly membership = new Map<string, MembershipOp[]>();
	/** Ops the run did not produce: they were on the agents' devices before
	 * it began, so whatever they once announced is not this run's to expect. */
	private readonly silent = new Set<OpId>();
	/** Agent name → the ops it had yet to hear when it came back to the
	 * foreground. Its app fetches those itself on the sync that follows and
	 * announces none of them; what arrives *while* a device is away is what
	 * makes a notification, and reaching an away device takes a push. */
	private readonly catchingUp = new Map<string, Set<OpId>>();
	/** Answers a request's arrival has called for, waiting to be written: they
	 * are made while knowledge is being spread, and adding to the ops being
	 * walked there would disturb the walk. */
	private pendingAccepts: Extract<Op, { kind: 'accept' }>[] = [];
	/** Blocks in force, as 'blocker>blocked'. Nothing of one leaves the
	 * blocker's own devices. */
	private readonly blocked = new Set<string>();
	/** Ops a device threw away as they arrived while it was blocking their
	 *  author — a profile, an invitation to a group: never offered again, so
	 *  never seen. */
	private readonly discarded = new Map<string, Set<OpId>>();
	/** Chat ops a device will not show because their author was blocked when
	 *  they were written — see [`hide`]. */
	private readonly hidden = new Map<string, Set<OpId>>();
	/** Agent name → chats whose view grew since a run last looked at its
	 * screen. An agent that was away gains while it is away; the chats it
	 * gained are checked when it can be driven again. */
	private readonly unchecked = new Map<string, Set<ExpectedChat>>();
	/** Agent name → the LAN it is on; absent while it is on none. */
	private readonly network = new Map<string, string>();
	private messageCounter = 0;
	private groupCounter = 0;
	private groupInfoCounter = 0;
	private profileCounter = 0;
	private membershipCounter = 0;
	private hubCounter = 0;

	constructor(
		agents: ExpectedAgent[],
		networks: ExpectedNetwork[] = [],
		cloudUsable = true,
		cloudDegradable = false,
		push = false,
	) {
		this.agents = agents;
		this.networks = networks;
		this.cloud = { usable: cloudUsable };
		this.cloudDegradable = cloudDegradable;
		this.push = push;
		this.knowledge.set(CLOUD, new Set());
		// Every holder exists before the first op is recorded: recording
		// converges, and converging reads the knowledge of all of them.
		for (const { name, notifications } of agents) {
			this.knowledge.set(name, new Set());
			// Only an agent whose device the run reads keeps notifications: for
			// the rest there is nothing to compare an expectation against, and
			// [`noteNotified`] leaves them alone by finding none of this.
			if (notifications !== null) this.posted.set(name, new Map());
		}
		for (const { name } of agents) this.recordProfile(name, name);
	}

	names(): string[] {
		return this.agents.map(a => a.name);
	}

	/** Agents whose UI can be driven right now. */
	activeNames(): string[] {
		return this.names().filter(n => this.isActive(n));
	}

	activeMobileNames(): string[] {
		return this.agents
			.filter(a => a.mobile && this.isActive(a.name))
			.map(a => a.name);
	}

	backgroundedNames(): string[] {
		return [...this.backgrounded];
	}

	/** Agents whose app is running, in the foreground or the background. */
	runningNames(): string[] {
		return this.names().filter(n => !this.stopped.has(n));
	}

	stoppedNames(): string[] {
		return [...this.stopped];
	}

	isActive(name: string): boolean {
		return !this.backgrounded.has(name) && !this.stopped.has(name);
	}

	isStopped(name: string): boolean {
		return this.stopped.has(name);
	}

	background(name: string): void {
		this.backgrounded.add(name);
		this.converge();
	}

	/** Coming back to the front reads the route the app returns to, which is
	 *  the one it was taken away from: what the device was showing for it and
	 *  what its row had counted are both gone. */
	foreground(name: string): void {
		// Only what was away has anything to catch up on: a move that
		// foregrounds an app already on screen must not silence it.
		if (this.backgrounded.delete(name)) this.catchUp(name);
		const route = this.viewing.get(name)?.route;
		this.converge();
		if (route === undefined) return;
		this.clearPosted(name, route);
		this.clearUnread(name, route);
	}

	stopApp(name: string): void {
		this.backgrounded.delete(name);
		this.stopped.add(name);
		this.converge();
	}

	/** Everything `name` has yet to hear is now a catch-up rather than an
	 *  announcement: what it missed while it was away, which its app fetches
	 *  quietly on the next sync. Anything written after this notifies.
	 *
	 *  The set is dropped at the end of that one `spread`, whether or not it
	 *  delivered anything — an agent that comes back onto no network and
	 *  rejoins later announces what it then fetches. That is the modelled
	 *  choice, not a measured one: if the app stays quiet through that second
	 *  sync too, this reports a notification the device never posts. */
	private catchUp(name: string): void {
		const known = this.knows(name);
		this.catchingUp.set(
			name,
			new Set(this.ops.filter(op => !known.has(op.id)).map(op => op.id)),
		);
	}

	/** An app that was stopped comes back on the chat list, not on whatever it
	 *  was showing when it went away. */
	startApp(name: string): void {
		if (this.stopped.delete(name)) this.catchUp(name);
		this.wentHome(name);
		this.converge();
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
		this.converge();
	}

	agentLeave(name: string): void {
		this.network.delete(name);
		this.converge();
	}

	/** Agents whose UI can be driven right now, on `network`. */
	activeNamesOn(network: string): string[] {
		return this.activeNames().filter(n => this.networkOf(n) === network);
	}

	networkNames(): string[] {
		return this.networks.map(n => n.name);
	}

	/** The LANs the hubs can be moved onto: every one but the home LAN. */
	hubNetworkNames(): string[] {
		return this.networks.filter(n => !n.home).map(n => n.name);
	}

	homeNetwork(): string | null {
		return this.networks.find(n => n.home)?.name ?? null;
	}

	hasNetworks(): boolean {
		return this.networks.length > 0;
	}

	/** Whether the run can take the cloud link down and bring it back. */
	hasCloud(): boolean {
		return this.cloudDegradable;
	}

	/** Whether agents reach the cloud mailbox right now. */
	cloudUsable(): boolean {
		return this.cloud.usable;
	}

	setCloudUsable(usable: boolean): void {
		this.cloud.usable = usable;
		this.converge();
	}

	/** Whether every agent has a members-less group to read its connection
	 * chip in: a run with LANs reads hubs on it, one with a cloud reads the
	 * cloud. */
	watchesChip(): boolean {
		return this.hasNetworks() || this.hasCloud();
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
		this.converge();
	}

	hubLeave(name: string): void {
		this.hub(name).network = null;
		this.converge();
	}

	startHub(name: string): void {
		this.hub(name).running = true;
		this.converge();
	}

	stopHub(name: string): void {
		this.hub(name).running = false;
		this.converge();
	}

	/** The running hubs on `network`. A hub is wherever the host's card is:
	 * the LAN it was moved onto, or the home LAN while it is on none. */
	private hubsOn(network: string): ExpectedHub[] {
		return this.hubs.filter(
			h => h.running && (h.network ?? this.homeNetwork()) === network,
		);
	}

	/** Hubs `name`'s app must show as connected: the running ones on its
	 * network, none while it is on no network. */
	expectedHubs(name: string): number {
		const network = this.networkOf(name);
		if (network === null) return 0;
		return this.hubsOn(network).length;
	}

	/** Whether the pair have each entered the other's link. A fact about the
	 *  two of them, which neither device need know yet: for what one of them
	 *  can act on, see [`contactsOf`]. */
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

	/** Record that `from` entered `to`'s add-contact link. The first of the
	 * pair to do so sends a request, which `to`'s device announces; answering
	 * one completes the pair and adds their chat to the model, and is an
	 * acceptance its sender is never told about. */
	recordAdded(from: string, to: string): void {
		const completes = this.added.has(`${to}>${from}`);
		this.added.add(`${from}>${to}`);
		// Both adds announce themselves the same way — entering a link is a
		// local act, and what tells the other side it happened is an op on
		// their inbox. The add that completes a pair is no exception: until
		// its op lands, the peer has only its own outgoing request, and shows
		// the pair as one.
		const id = requestOp(from, to);
		const name = this.displayName(from, from) ?? from;
		this.requestNames.set(id, name);
		this.record(from, {
			id,
			topic: `inbox:${to}`,
			kind: 'request',
			from,
			to,
			name,
		});
		if (!completes) return;
		this.addChat({
			kind: 'direct',
			id: '',
			name: '',
			creator: '',
			members: [from, to],
		});
	}

	/**
	 * Accept a request `accepter` is holding, as tapping accept does: the node
	 * writes the contact marker on its own device there and then — no round
	 * trip, unlike entering a link — and replies, which is what lets the
	 * requester write theirs. `accept_contact` in
	 * crates/dashchat-node/src/node.rs, reached both by the tap and, for a
	 * mutual add, by [`noteAccepted`] when the second request arrives.
	 */
	recordAccepted(accepter: string, requester: string): void {
		this.added.add(`${accepter}>${requester}`);
		this.record(accepter, {
			id: acceptOp(accepter, requester),
			topic: `inbox:${requester}`,
			kind: 'accept',
			from: accepter,
			to: requester,
		});
		if (this.directChatOrNull(accepter, requester) !== null) return;
		this.addChat({
			kind: 'direct',
			id: '',
			name: '',
			creator: '',
			members: [accepter, requester],
		});
	}

	/** Whether `name` is blocking `peer`. */
	blocks(name: string, peer: string): boolean {
		return this.blocked.has(`${name}>${peer}`);
	}

	/** Contacts `name` can block: the ones it is not blocking already. */
	blockablePeers(name: string): string[] {
		return this.contactsOf(name);
	}

	/** Contacts `name` is blocking, which are the ones it can unblock. */
	blockedPeers(name: string): string[] {
		return this.names().filter(other => this.blocks(name, other));
	}

	/** Record that `name` blocked `peer`: from here their operations are
	 *  thrown away as they arrive, the pair's chat is read-only, and no
	 *  picker offers them. */
	blockContact(name: string, peer: string): void {
		this.blocked.add(`${name}>${peer}`);
		this.converge();
	}

	/** Record that `name` unblocked `peer`. What arrived while the block was
	 *  on stays thrown away: only what comes after is kept — see [`rejects`]. */
	unblockContact(name: string, peer: string): void {
		this.blocked.delete(`${name}>${peer}`);
		this.converge();
	}

	/** Requests `name` can accept: someone added them, their request has
	 *  arrived, and `name` has not added them back. */
	pendingRequestsFor(name: string): string[] {
		return this.names().filter(
			other =>
				other !== name &&
				!this.added.has(`${name}>${other}`) &&
				this.knows(name).has(requestOp(other, name)),
		);
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
				this.hasAccepted(name, other) &&
				!this.blocks(name, other) &&
				this.knowsProfile(name, other),
		);
	}

	/** Chats `name` has heard of, whether or not it is still in them. A group
	 * goes on syncing after a departure, which is how the departure itself
	 * reaches the members who stay. */
	private knownChats(name: string): ExpectedChat[] {
		return this.chats.filter(c =>
			c.kind === 'direct' ? c.members.includes(name) : this.hasGroup(name, c),
		);
	}

	/** Chats `name` can open: the direct chats of its contacts, and the
	 * groups it has learnt about and is still in as far as it knows. */
	chatsFor(name: string): ExpectedChat[] {
		return this.chats.filter(
			c =>
				this.membersFor(c, name).includes(name) &&
				(c.kind === 'direct'
					? this.hasDirectChat(name, c)
					: this.hasGroup(name, c)),
		);
	}

	/** Whether `name` has a row for a direct chat: it entered the peer's link,
	 *  which leaves a placeholder of its own, or the peer's request reached it.
	 *  A chat the pair will share once an answer crosses is on neither screen
	 *  before that. */
	private hasDirectChat(name: string, chat: ExpectedChat): boolean {
		const peer = chat.members.find(member => member !== name);
		if (peer === undefined) return true;
		return (
			this.added.has(`${name}>${peer}`) ||
			this.knows(name).has(requestOp(peer, name))
		);
	}

	/** Who `viewer` has in `chat`: the members it was created with, plus the
	 * joins and minus the departures that have reached that device. A
	 * removal travels like anything else, so someone who was away still has
	 * the group until it arrives. */
	membersFor(chat: ExpectedChat, viewer: string): string[] {
		if (chat.kind === 'direct') return chat.members;
		const known = this.knows(viewer);
		const members = new Set(chat.members);
		for (const op of this.membership.get(chat.id) ?? []) {
			if (!known.has(op.id)) continue;
			if (op.kind === 'group') members.add(op.member);
			else members.delete(op.member);
		}
		return [...members];
	}

	/** Chats `name` can send in: the ones it can open whose composer is there.
	 * Only a block takes it away — a group it has left is not among the chats
	 * it can open at all. */
	sendableChatsFor(name: string): ExpectedChat[] {
		return this.chatsFor(name).filter(c => !this.view(name, c).blocked);
	}

	/** Create a group: the creator knows it at once; each invited member
	 * learns of it through its inbox. */
	addGroup(creator: string, members: string[], name: string): ExpectedChat {
		const chat: ExpectedChat = {
			kind: 'group',
			id: name,
			name,
			creator,
			members: [creator, ...members],
		};
		this.addChat(chat);
		for (const member of members) {
			this.recordMembership(creator, this.groupAddOp(chat, member, creator));
		}
		return chat;
	}

	/** The members-less group `name` reads its connection chip in, which
	 *  preparation makes and moves leave alone; null in a run with no chip to
	 *  read. */
	chipChatOf(name: string): ExpectedChat | null {
		return this.chipChats.get(name) ?? null;
	}

	/** Record the group preparation made for `name` to read its chip in. */
	setChipChat(name: string, chat: ExpectedChat): void {
		this.chipChats.set(name, chat);
	}

	/** Groups `name` created and may act on: its chip chat is the run's own
	 *  fixture, not its subject, so nothing renames or leaves it. */
	groupsCreatedBy(name: string): ExpectedChat[] {
		return this.chatsFor(name).filter(
			chat =>
				chat.kind === 'group' &&
				chat.creator === name &&
				chat !== this.chipChatOf(name),
		);
	}

	/** Contacts of `name` that `chat` does not already have. */
	addableTo(chat: ExpectedChat, name: string): string[] {
		const members = this.membersFor(chat, name);
		return this.contactsOf(name).filter(contact => !members.includes(contact));
	}

	/** Members of `chat` other than `name` itself. */
	removableFrom(chat: ExpectedChat, name: string): string[] {
		return this.membersFor(chat, name).filter(member => member !== name);
	}

	/** Record that `by` added `member` to `chat`: the member learns of the
	 *  group through their inbox, as an invited one does. */
	addGroupMember(chat: ExpectedChat, by: string, member: string): void {
		this.recordMembership(by, this.groupAddOp(chat, member, by));
	}

	/** Groups `name` may leave: any it is in, except one it created that
	 *  others are still in — the app refuses to let a group's last admin go. */
	leavableGroups(name: string): ExpectedChat[] {
		return this.chatsFor(name).filter(
			chat =>
				chat.kind === 'group' &&
				chat !== this.chipChatOf(name) &&
				(chat.creator !== name || this.membersFor(chat, name).length === 1),
		);
	}

	/** Record that `name` left `chat`, which is removing yourself. */
	leaveGroup(chat: ExpectedChat, name: string): void {
		this.removeGroupMember(chat, name, name);
	}

	/** Record that `by` took `member` out of `chat`. It goes on the group's
	 *  own topic, as the joins do, so every member — the one removed
	 *  included — stops counting them once it arrives, and nobody is told
	 *  about it on their shade. */
	removeGroupMember(chat: ExpectedChat, by: string, member: string): void {
		this.recordMembership(by, {
			id: `groupRemoved:${chat.id}>${member}:${++this.membershipCounter}`,
			topic: chatTopic(chat),
			kind: 'groupRemoved',
			chat,
			member,
			by,
		});
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
		photos?: number,
	): ExpectedMessage {
		const message: ExpectedMessage = {
			label,
			kind,
			sender,
			chat,
			replyTo,
			hasReply: false,
			edits: 0,
			photos,
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

	/** The chat `viewer` has under `title` in its list, or null for a title
	 *  its model knows nothing about. */
	chatByName(title: string, viewer: string): ExpectedChat | null {
		return (
			this.chatsFor(viewer).find(
				chat => this.chatListName(chat, viewer) === title,
			) ?? null
		);
	}

	/** How `chat` appears in `viewer`'s chat list: a group under the last
	 *  name change that reached it, a direct chat under its peer's. */
	chatListName(chat: ExpectedChat, viewer: string): string {
		if (chat.kind === 'group') return this.groupInfo(chat, viewer).name;
		const peer = chat.members.find(m => m !== viewer);
		if (peer === undefined) {
			throw new Error(`direct chat has no peer for ${viewer}`);
		}
		return this.displayName(viewer, peer) ?? peer;
	}

	/** What `viewer` calls `who`: the last profile of theirs to reach it, or
	 *  the one their contact request carried while that is all it has, or null
	 *  while it has neither and only their key to go on. */
	displayName(viewer: string, who: string): string | null {
		const known = this.knows(viewer);
		let name: string | null = null;
		for (const op of this.profiles.get(who) ?? []) {
			if (op.kind === 'profile' && known.has(op.id)) name = op.name;
		}
		if (name !== null) return name;
		const request = requestOp(who, viewer);
		if (!known.has(request)) return null;
		return this.requestNames.get(request) ?? null;
	}

	/** Whether `viewer` has any profile of `who`. */
	knowsProfile(viewer: string, who: string): boolean {
		return this.displayName(viewer, who) !== null;
	}

	/** Give `viewer` the profiles `who` has published: what meeting before
	 *  the run began left on their devices. */
	private markProfileKnown(viewer: string, who: string): void {
		for (const op of this.profiles.get(who) ?? []) {
			this.knowledge.get(viewer)?.add(op.id);
		}
	}

	/** Record that `agent` gave itself a new name, which reaches everyone
	 *  that syncs its announcements. */
	updateProfile(agent: string, name: string): void {
		this.recordProfile(agent, name);
	}

	/** Zero-padded so `person-01` can never substring-match `person-010`, and
	 *  of a family no group name contains: a row, a picker and a
	 *  notification are read by what they say, so no two things a run can
	 *  name may be called the same, or one the same as part of another. */
	nextProfileName(): string {
		return `person-${String(++this.profileCounter).padStart(2, '0')}`;
	}

	/** The name and description `viewer` has for `chat`: the last change to
	 *  reach it, or what the group was created with. */
	groupInfo(
		chat: ExpectedChat,
		viewer: string,
	): { name: string; description: string } {
		const known = this.knows(viewer);
		let info = { name: chat.name, description: '' };
		for (const op of this.opsByTopic.get(chatTopic(chat)) ?? []) {
			if (op.kind !== 'groupInfo' || !known.has(op.id)) continue;
			info = { name: op.name, description: op.description };
		}
		return info;
	}

	/** Record that `chat`'s creator gave it a new name and description, which
	 *  reach the members through the group's own topic. */
	setGroupInfo(chat: ExpectedChat, name: string, description: string): void {
		this.record(chat.creator, {
			id: `groupInfo:${chat.id}:${++this.groupInfoCounter}`,
			topic: chatTopic(chat),
			kind: 'groupInfo',
			chat,
			name,
			description,
		});
	}

	/** Zero-padded so `renamed-01` can never substring-match `renamed-010`. */
	nextGroupInfo(): { name: string; description: string } {
		const n = String(this.groupInfoCounter + 1).padStart(2, '0');
		return { name: `renamed-${n}`, description: `about-${n}` };
	}

	/** Recent live text messages `name` sees and may interact with (react,
	 * reply, and — with `ownOnly` — edit or delete), across its chats. */
	interactionTargets(name: string, ownOnly = false): InteractionTarget[] {
		return this.sendableChatsFor(name).flatMap(chat =>
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
			if (!known.has(op.id) || this.isHidden(agent, op)) continue;
			this.fold(views, op);
		}
		// Who the peer is, not what this device calls them: a rename changes
		// the title of the row, never which agent is behind it.
		const peer =
			chat.kind === 'direct'
				? (chat.members.find(member => member !== agent) ?? null)
				: null;
		return {
			blocked: peer !== null && this.blocks(agent, peer),
			departed:
				chat.kind === 'group' && !this.membersFor(chat, agent).includes(agent),
			messages: [...views.values()],
		};
	}

	/**
	 * Record a pair the agents are already contacts of, without the request
	 * that made them so: it was sent before the run started, and whatever it
	 * put on their devices is not this run's to expect.
	 */
	recordExistingContacts(a: string, b: string): void {
		this.added.add(`${a}>${b}`);
		this.added.add(`${b}>${a}`);
		// A pair read off the devices are contacts there already, marker and
		// all, so both hold the answer that made them one. Without it nothing
		// the run adopts would count as a contact and every move that needs
		// one — a group, a block — would quietly have nobody to make it with.
		const accepted = acceptOp(a, b);
		this.silent.add(accepted);
		this.knowledge.get(a)?.add(accepted);
		this.knowledge.get(b)?.add(accepted);
		if (this.directChatOrNull(a, b) !== null) return;
		this.addChat({
			kind: 'direct',
			id: '',
			name: '',
			creator: '',
			members: [a, b],
		});
		// Their profiles reached each other when they met, or the chat would
		// not be showing a name to have been read off.
		this.markProfileKnown(a, b);
		this.markProfileKnown(b, a);
	}

	/** Record a group the agents are already in, its first member standing in
	 *  for whoever created it. */
	recordExistingGroup(name: string, members: string[]): ExpectedChat {
		this.groupCounter = Math.max(this.groupCounter, suffixNumber(name));
		const existing = this.chats.find(
			c => c.kind === 'group' && c.name === name,
		);
		if (existing !== undefined) return existing;
		const chat: ExpectedChat = {
			kind: 'group',
			id: name,
			name,
			creator: members[0],
			members,
		};
		this.addChat(chat);
		for (const member of members) {
			// The run neither produced these adds nor may expect a device to
			// announce them, as `store` does for everything else it adopts.
			// Silencing comes first: recording is what delivers an op.
			const op = this.groupAddOp(chat, member, chat.creator);
			this.silent.add(op.id);
			this.recordMembership(member, op);
		}
		return chat;
	}

	/**
	 * Record a message already in `chat`, known by the agents whose screen
	 * showed it. It notifies nobody: it was on their devices
	 * before the run started, so the run neither expects a notification for
	 * it nor may blame one on it.
	 */
	recordExistingMessage(
		chat: ExpectedChat,
		sender: string,
		kind: MessageKind,
		label: string,
		knowers: string[],
		state: { deleted: boolean; reactions: string[] },
	): void {
		this.messageCounter = Math.max(
			this.messageCounter,
			kind === 'voice' ? durationSeconds(label) : suffixNumber(label),
		);
		if (this.messages.has(label)) {
			this.markKnown(knowers, label);
			return;
		}
		const message: ExpectedMessage = {
			label,
			kind,
			sender,
			chat,
			hasReply: false,
			edits: 0,
		};
		this.messages.set(label, message);
		const topic = chatTopic(chat);
		this.store(sender, {
			id: `message:${label}`,
			topic,
			kind: 'message',
			message,
		});
		if (kind !== 'text') {
			this.store(sender, {
				id: `bytes:${label}`,
				topic,
				kind: 'bytes',
				message,
			});
		}
		if (state.deleted) {
			this.store(sender, {
				id: `delete:${label}`,
				topic,
				kind: 'delete',
				message,
			});
		}
		for (const emoji of state.reactions) {
			// A screen shows which reactions a message carries but not whose
			// they are, and only the set of them is ever compared.
			const reactor = `before:${emoji}`;
			this.store(sender, {
				id: `reaction:${label}:${reactor}:${emoji}`,
				topic,
				kind: 'reaction',
				message,
				reactor,
				emoji,
			});
		}
		this.markKnown(knowers, label);
	}

	/** Give every one of `names` the ops of the message labelled `label`. */
	private markKnown(names: string[], label: string): void {
		for (const op of this.ops) {
			if (!('message' in op) || op.message.label !== label) continue;
			for (const name of names) this.knowledge.get(name)?.add(op.id);
		}
	}

	/** What `name`'s device must be showing right now. */
	expectedNotifications(name: string): NotificationView[] {
		return [...(this.posted.get(name)?.values() ?? [])];
	}

	/** Mobile agents holding notifications: the devices a run can read and
	 *  tap. */
	notifiedNames(): string[] {
		return this.agents
			.filter(a => a.mobile && this.expectedNotifications(a.name).length > 0)
			.map(a => a.name);
	}

	/** The chat `name` has open, or null while it is on the chat list or on
	 *  one the model cannot name yet. */
	viewingChat(name: string): ExpectedChat | null {
		return this.viewing.get(name)?.chat ?? null;
	}

	/** Record that `name` opened `chat`: it is now looking at it, and its app
	 *  clears whatever it had posted for that route and reads everything that
	 *  had piled up unread in it. */
	openedChat(name: string, chat: ExpectedChat): void {
		const route = chatTopic(chat);
		this.viewing.set(name, { route, chat });
		this.clearPosted(name, route);
		this.clearUnread(name, route);
	}

	/** Record that `name` is back on the chat list. */
	wentHome(name: string): void {
		this.viewing.delete(name);
	}

	/** Record that `name` opened its direct chat with `peer`, which entering
	 *  their contact link does — even before the pair are contacts and the
	 *  chat itself is in the model, which is when it clears the request
	 *  `peer` sent. */
	openedDirectChat(name: string, peer: string): void {
		const route = directTopic(name, peer);
		this.viewing.set(name, { route, chat: this.directChatOrNull(name, peer) });
		this.clearPosted(name, route);
		this.clearUnread(name, route);
	}

	/** How many unread messages `name`'s chat list must show on `chat`'s row. */
	unreadCount(name: string, chat: ExpectedChat): number {
		return this.unread.get(name)?.get(chatTopic(chat))?.size ?? 0;
	}

	/**
	 * Spread knowledge as the app does: within each LAN, for each topic, the
	 * holders subscribed to it end up with the union of their ops of it. A hub
	 * subscribes to everything. Iterated until nothing changes, since learning
	 * a group subscribes to its chat.
	 *
	 * Every recording and every change of who can reach whom ends in this, so
	 * an op is in the model's hands the moment it is deliverable — as it is in
	 * the app's. A check that read a model still holding one back would be
	 * waiting for a device to un-receive it.
	 */
	private converge(): boolean {
		return this.spread(() => this.components());
	}

	/** Converge and hand over what grew, which is what a move's settle does. */
	propagate(): Map<string, ExpectedChat[]> {
		this.converge();
		return this.takeGrowth();
	}

	/** Spread knowledge as if every agent shared one LAN: where they are
	 * after being together on their usual network, as during preparation. */
	propagateShared(): Map<string, ExpectedChat[]> {
		this.spread(() => [this.activeNames()]);
		return this.takeGrowth();
	}

	/**
	 * Throw when the model was holding ops back that the devices may already
	 * have: `where` asserted against a state they had left behind. Always true
	 * while every mutator converges — a failure here names the one that does
	 * not, rather than leaving it to surface as a phantom product bug.
	 */
	assertConverged(where: string): void {
		if (this.converge()) {
			throw new Error(
				`${where} ran with the model behind the devices: ops became ` +
					'deliverable without the model converging on them',
			);
		}
	}

	/**
	 * Hand out the chats whose view grew since they were last taken, per agent
	 * a run can look at: one that was away kept gaining while it was, and what
	 * it gained is handed over once its screen is back.
	 */
	takeGrowth(): Map<string, ExpectedChat[]> {
		const growth = new Map<string, ExpectedChat[]>();
		for (const name of this.activeNames()) {
			const waiting = this.unchecked.get(name);
			if (waiting === undefined) continue;
			this.unchecked.delete(name);
			const open = this.chatsFor(name);
			const chats = this.chats.filter(c => waiting.has(c) && open.includes(c));
			if (chats.length > 0) growth.set(name, chats);
		}
		return growth;
	}

	/**
	 * Bring knowledge up to date with everything deliverable so far, keeping
	 * what each agent gained for [`takeGrowth`]. Returns whether anything
	 * moved, which is how a check tells it was about to assert against a state
	 * the devices had already left behind.
	 */
	private spread(components: () => string[][]): boolean {
		const before = new Map(
			this.names().map(name => [name, new Set(this.knows(name))]),
		);
		let moved = false;
		let changed = true;
		while (changed) {
			changed = false;
			for (const component of components()) {
				if (this.unionComponent(component)) changed = moved = true;
			}
			// An answer written here is itself something to deliver, so the
			// loop goes round again for it.
			if (this.writeAccepts()) changed = moved = true;
		}
		// Whatever was waiting has landed, silently; anything arriving from
		// here on reaches an app that is up and announces itself.
		this.catchingUp.clear();
		for (const name of this.names()) {
			const gained = this.chatsGained(name, before.get(name) ?? new Set());
			if (gained.length === 0) continue;
			const waiting = this.unchecked.get(name) ?? new Set<ExpectedChat>();
			for (const chat of gained) waiting.add(chat);
			this.unchecked.set(name, waiting);
		}
		return moved;
	}

	private message(label: string): ExpectedMessage {
		const found = this.messages.get(label);
		if (found === undefined) throw new Error(`no message labelled ${label}`);
		return found;
	}

	private hasGroup(name: string, chat: ExpectedChat): boolean {
		if (chat.creator === name) return true;
		const known = this.knows(name);
		return (this.membership.get(chat.id) ?? []).some(
			op => op.kind === 'group' && op.member === name && known.has(op.id),
		);
	}

	/** A chat is a topic its members subscribe to from here on, so whatever
	 *  was already written to it can travel the moment the chat exists. */
	private addChat(chat: ExpectedChat): void {
		this.chats.push(chat);
		this.chatByTopic.set(chatTopic(chat), chat);
		this.converge();
	}

	private record(holder: string, op: Op): void {
		this.put(holder, op);
	}

	/** Record a profile of `agent` under `name`: what every device that has
	 *  heard it calls them from then on. */
	private recordProfile(agent: string, name: string): void {
		const versions = this.profiles.get(agent) ?? [];
		const op: Op = {
			id: profileOp(agent, versions.length),
			topic: `announce:${agent}`,
			kind: 'profile',
			agent,
			name,
		};
		versions.push(op);
		this.profiles.set(agent, versions);
		this.record(agent, op);
	}

	/** Record that `by` put `member` in `chat`, as its own op: a member taken
	 *  out and put back is added by an op no device has seen before, which is
	 *  what makes the second add news rather than something already known. */
	private groupAddOp(
		chat: ExpectedChat,
		member: string,
		by: string,
	): MembershipOp {
		return {
			id: `group:${chat.id}>${member}:${++this.membershipCounter}`,
			topic: `inbox:${member}`,
			kind: 'group',
			chat,
			member,
			by,
		};
	}

	/** Record a join or a departure, and keep it under its group in the order
	 *  it happened: membership is those folded in turn. */
	private recordMembership(holder: string, op: MembershipOp): void {
		this.record(holder, op);
		const changes = this.membership.get(op.chat.id);
		if (changes === undefined) this.membership.set(op.chat.id, [op]);
		else changes.push(op);
	}

	/** Put an op in the log and on `holder`, announcing nothing wherever it
	 *  reaches — what adopting state the agents already had does, since the
	 *  run neither produced it nor may expect anything on a device for it. */
	private store(holder: string, op: Op): void {
		this.silent.add(op.id);
		this.put(holder, op);
	}

	private put(holder: string, op: Op): void {
		this.write(holder, op);
		// An op is on its way to everyone it can reach the moment it exists —
		// unless it is state the run adopted rather than produced, which is a
		// fixture of who had what and stays where it was put.
		if (!this.silent.has(op.id)) this.converge();
	}

	/** Write `op` down as `holder`'s, without spreading it: everything [`put`]
	 *  does but the converge, for a caller already inside one. */
	private write(holder: string, op: Op): void {
		this.ops.push(op);
		const ops = this.opsByTopic.get(op.topic);
		if (ops === undefined) this.opsByTopic.set(op.topic, [op]);
		else ops.push(op);
		this.knowledge.get(holder)?.add(op.id);
		this.hide(op);
	}

	/**
	 * Answer `op` when it is a request from someone `holder` had already
	 * entered the link of. Entering a link leaves only a placeholder on one's
	 * own device (`PendingContactRequest`); the contact marker a chat list is
	 * built from is written by whoever *receives* the second request, and the
	 * reply that goes with it is what lets the other side write theirs. So a
	 * pair are contacts on no device until a request has crossed and an answer
	 * has come back — which is why entering a link into a dead network makes
	 * no contact at all, on either side. See `add_contact` and `accept_contact`
	 * in crates/dashchat-node/src/node.rs.
	 */
	private noteAccepted(holder: string, op: Op): void {
		if (op.kind !== 'request' || op.to !== holder) return;
		if (!this.added.has(`${holder}>${op.from}`)) return;
		const id = acceptOp(holder, op.from);
		if (this.knows(holder).has(id)) return;
		this.pendingAccepts.push({
			id,
			topic: `inbox:${op.from}`,
			kind: 'accept',
			from: holder,
			to: op.from,
		});
	}

	/** Write the answers [`noteAccepted`] called for, on the device that
	 *  answers; returns whether there were any. */
	private writeAccepts(): boolean {
		if (this.pendingAccepts.length === 0) return false;
		const accepts = this.pendingAccepts;
		this.pendingAccepts = [];
		for (const op of accepts) this.write(op.from, op);
		return true;
	}

	/** Whether `name`'s device holds the contact marker for `other`: the answer
	 *  that made the pair, whichever of them wrote it, has reached it. */
	private hasAccepted(name: string, other: string): boolean {
		const known = this.knows(name);
		return known.has(acceptOp(name, other)) || known.has(acceptOp(other, name));
	}

	/**
	 * Note, as `op` is written, every agent that was blocking its author right
	 * then: their screens filter a chat op out by the time it was authored, so
	 * unblocking never brings it back and an op that arrives long afterwards is
	 * filtered just the same (`dropOpsAuthoredWhileBlocked` in
	 * packages/stores/src/chats/messages-store.ts). A group's info is exempt
	 * there, as it is in the node's own blocklist.
	 */
	private hide(op: Op): void {
		if (!CHAT_KINDS.includes(op.kind)) return;
		const author = authorOf(op);
		for (const name of this.names()) {
			if (!this.blocks(name, author)) continue;
			const set = this.hidden.get(name);
			if (set === undefined) this.hidden.set(name, new Set([op.id]));
			else set.add(op.id);
		}
	}

	/** Whether `viewer`'s screen leaves `op` out although its device has it. */
	private isHidden(viewer: string, op: Op): boolean {
		return this.hidden.get(viewer)?.has(op.id) === true;
	}

	/** Post what `op` shows on `holder`'s device, now that it has arrived
	 *  there — which is when the app builds its notification, so an op still
	 *  on its way announces nothing. The app suppresses one only for the
	 *  route an agent is looking at right now: a backgrounded app is looking
	 *  at nothing, so it is notified even for the chat it was left on. */
	private noteNotified(holder: string, op: Op): void {
		if (this.catchingUp.get(holder)?.has(op.id) === true) return;
		if (this.silent.has(op.id) || !this.notifies(holder, op)) return;
		// A notification is the node's doing, and the node turns away what
		// arrives while the author is blocked — a different rule from the one
		// the chat filters its own contents by (see [`hide`]).
		if (this.blocks(holder, authorOf(op))) return;
		const notification = this.notificationFor(holder, op);
		if (notification === null) return;
		const viewing = this.isActive(holder)
			? this.viewing.get(holder)
			: undefined;
		if (viewing?.route === notification.route) return;
		const posted = this.posted.get(holder);
		if (posted === undefined) return;
		// A chat's messages share an entry, and each arrival rewrites it — so
		// what it reads is one of them, not necessarily the last to arrive.
		const shown = posted.get(notification.id);
		posted.set(notification.id, {
			...notification,
			oneOf: [...(shown?.oneOf ?? []), ...notification.oneOf],
		});
	}

	/** Count `op` towards `chat`'s badge on `holder`'s list, now that it has
	 *  arrived there: a message someone else sent to a chat it is in and whose
	 *  page its app is not showing. An app away from the foreground is showing
	 *  nothing, so what lands there counts even for the chat it was left on.
	 *  Unlike a notification, this does not wait on the sender having been
	 *  added back — a row counts every message in its chat. */
	private noteUnread(holder: string, op: Op): void {
		if (op.kind !== 'message' || this.silent.has(op.id)) return;
		if (this.isHidden(holder, op)) return;
		const { chat, sender } = op.message;
		if (sender === holder) return;
		if (!this.membersFor(chat, holder).includes(holder)) return;
		const route = chatTopic(chat);
		if (this.isActive(holder) && this.viewing.get(holder)?.route === route) {
			return;
		}
		const byRoute = this.unread.get(holder) ?? new Map<Topic, Set<OpId>>();
		this.unread.set(holder, byRoute);
		const counted = byRoute.get(route) ?? new Set<OpId>();
		byRoute.set(route, counted);
		counted.add(op.id);
	}

	/** Drop what `name`'s row for a route had counted, as its app does when
	 *  the chat is entered and everything in it is marked read. */
	private clearUnread(name: string, route: Topic): void {
		this.unread.get(name)?.delete(route);
	}

	/** Whether `op` is one `holder`'s app announces. A group's messages reach
	 *  every member; a direct chat's only once its sender has been added back,
	 *  which is what the app takes as accepting them. A request and an invite
	 *  are each addressed to one agent, however many peers sync the topic they
	 *  are on. */
	private notifies(holder: string, op: Op): boolean {
		if (op.kind === 'message') {
			const { chat, sender } = op.message;
			if (sender === holder) return false;
			if (!this.membersFor(chat, holder).includes(holder)) return false;
			return chat.kind === 'group' || this.added.has(`${holder}>${sender}`);
		}
		if (op.kind === 'request') return op.to === holder;
		// Being added to a group is announced; being removed from one is not,
		// so only the chat it takes away is expected of it.
		if (op.kind === 'group') return op.member === holder;
		return false;
	}

	/** The notification an op produces on `holder`'s device, or null for one
	 *  the app shows nothing for. A chat's messages share an id, so a later
	 *  one replaces the entry the way the device collapses a conversation;
	 *  every other kind gets its own. */
	private notificationFor(
		holder: string,
		op: Op,
	): (NotificationView & { id: string }) | null {
		if (op.kind === 'message') {
			return {
				id: `chat:${chatTopic(op.message.chat)}`,
				route: chatTopic(op.message.chat),
				opens: op.message.chat,
				shows: this.nameShown(holder, op.message.sender),
				oneOf: bodyOf(op.message, this.textsFor(holder)),
			};
		}
		if (op.kind === 'request') {
			// An agent syncs the inbox of every peer it has added, so requests
			// other people send them arrive on its device too. Only the inbox's
			// owner announces what is in it.
			if (op.to !== holder) return null;
			// The add that completes a pair reaches the other's inbox the same
			// way, but it answers a request they already made rather than
			// making one: they learn they have a chat by having one.
			if (this.added.has(`${op.to}>${op.from}`)) return null;
			// "New contact request / <name>", the name taken from the profile
			// the request carries rather than from what the device knows.
			return {
				id: op.id,
				route: directTopic(op.from, op.to),
				opens: this.directChatOrNull(op.from, op.to),
				shows: [op.name],
				oneOf: [],
			};
		}
		if (op.kind === 'group') {
			// "<who> added you to the group", under a title that is the group's
			// name only once its name op has arrived too.
			return {
				id: op.id,
				route: chatTopic(op.chat),
				opens: op.chat,
				shows: this.nameShown(holder, op.by),
				oneOf: [],
			};
		}
		return null;
	}

	/** `who`'s name, as far as a notification on `holder`'s device carries it:
	 *  the app has only their key until their profile arrives, and falls back
	 *  to wording that names nobody. */
	private nameShown(holder: string, who: string): string[] {
		const name = this.displayName(holder, who);
		if (name !== null) return [name];
		const texts = this.textsFor(holder);
		return texts === null ? [] : [texts.unnamedSender];
	}

	/** The wording `holder`'s app uses, or null for an agent the run does not
	 * read. */
	private textsFor(holder: string): NotificationTexts | null {
		return this.agents.find(a => a.name === holder)?.notifications ?? null;
	}

	/** Drop what `name`'s device was showing for a route, as its app does when
	 *  it navigates there. */
	private clearPosted(name: string, route: Topic): void {
		const posted = this.posted.get(name);
		if (posted === undefined) return;
		for (const [id, notification] of posted) {
			if (notification.route === route) posted.delete(id);
		}
	}

	private directChatOrNull(a: string, b: string): ExpectedChat | null {
		return (
			this.chats.find(
				c =>
					c.kind === 'direct' && c.members.includes(a) && c.members.includes(b),
			) ?? null
		);
	}

	private fold(views: Map<string, MessageView>, op: Op): void {
		if (
			op.kind === 'profile' ||
			op.kind === 'group' ||
			op.kind === 'groupRemoved' ||
			op.kind === 'groupInfo' ||
			op.kind === 'request' ||
			op.kind === 'accept'
		) {
			return;
		}
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

	/** Topics `holder` syncs, or null for a hub or the cloud, which sync them
	 * all. */
	private subscriptions(holder: string): Set<Topic> | null {
		if (holder === CLOUD || this.hubs.some(h => h.name === holder)) return null;
		const topics = new Set<Topic>([`announce:${holder}`, `inbox:${holder}`]);
		for (const other of this.names()) {
			if (!this.added.has(`${holder}>${other}`)) continue;
			topics.add(`announce:${other}`);
			topics.add(`inbox:${other}`);
		}
		for (const chat of this.knownChats(holder)) topics.add(chatTopic(chat));
		return topics;
	}

	private syncsDirectly(name: string): boolean {
		return this.agents.find(a => a.name === name)?.p2p !== false;
	}

	/** The holders that can sync with each other right now, grouped: per LAN,
	 * plus everyone with the cloud while its link is usable, plus the phones
	 * a push reaches. An agent away from the foreground is on no LAN — the OS
	 * cuts a backgrounded app off within seconds, and a stopped one is not
	 * there at all. */
	private components(): string[][] {
		const direct = (names: string[]) =>
			names.filter(n => this.syncsDirectly(n));
		const lans = this.hasNetworks()
			? this.networks.map(n => [
					...this.hubsOn(n.name).map(h => h.name),
					...direct(this.activeNamesOn(n.name)),
				])
			: [direct(this.activeNames())];
		const cloud = this.cloudUsable() ? [[CLOUD, ...this.activeNames()]] : [];
		return [...lans, ...cloud, ...this.pushed()];
	}

	/** What a push carries to the phones that are away from the foreground:
	 *  it wakes the app, backgrounded or killed, to fetch the operation it is
	 *  about, which is any the run's mailbox holds. Only a run whose mailbox
	 *  forwards pushes has them, and only while its link is usable — and only
	 *  for a phone the mailbox has something for, since a push is what the
	 *  arrival of an operation sends. An app nothing wakes writes nothing out
	 *  either: what it composed before it went stays on it. */
	private pushed(): string[][] {
		if (!this.push) return [];
		if (!this.cloudUsable()) return [];
		const away = this.agents
			.filter(
				a =>
					a.mobile &&
					!this.isActive(a.name) &&
					this.hasInternet(a.name) &&
					this.cloudHasNewsFor(a.name),
			)
			.map(a => a.name);
		if (away.length === 0) return [];
		// A woken app fetches from the mailbox; its peers reach it through the
		// cloud component, so joining them here would put agents that share no
		// LAN in one.
		return [[CLOUD, ...away]];
	}

	/** Whether the mailbox holds an operation `name` subscribes to and has not
	 *  seen: what there is to push, and so what there is to wake it for. */
	private cloudHasNewsFor(name: string): boolean {
		const topics = this.subscriptions(name);
		if (topics === null) return false;
		const known = this.knows(name);
		const cloud = this.knows(CLOUD);
		for (const topic of topics) {
			for (const op of this.opsByTopic.get(topic) ?? []) {
				if (cloud.has(op.id) && !known.has(op.id)) return true;
			}
		}
		return false;
	}

	/** Whether `name`'s device has a way off its own LAN. A run that walks
	 *  agents between networks leaves one on none at a time, and there it
	 *  reaches nothing at all. */
	private hasInternet(name: string): boolean {
		return !this.hasNetworks() || this.networkOf(name) !== null;
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
				if (known.has(op.id) || this.rejects(holder, op)) continue;
				known.add(op.id);
				this.noteAccepted(holder, op);
				this.noteNotified(holder, op);
				this.noteUnread(holder, op);
				changed = true;
			}
		}
		return changed;
	}

	/**
	 * Whether `holder`'s node turns `op` away rather than keeping it: what
	 * reaches a device while it is blocking the author is invalidated there,
	 * and unblocking never brings it back — not the messages, not a rename.
	 * The blocker carries on from what it knew when the block went on, and
	 * only what the author writes afterwards reaches it. An unblock that beats
	 * an op's arrival is not this case at all: nothing was invalidated, and it
	 * lands like any other.
	 *
	 * This is the app on a device, which is what the fuzz drives; the node
	 * library on its own does hand a held-back message back, since an
	 * invalidated op is never persisted and its author's log offers it again
	 * (`test_unblocking_brings_back_what_was_written_during_the_block`). What
	 * the app puts around it — its own stores, cursors and config — stops that
	 * from reaching a screen, so `blocked-messages-after-unblock.spec.ts` is
	 * the behaviour to model.
	 *
	 * TODO: this whole shape changes when the groups are properly encrypted.
	 * Today a block and a removal are only what a device does *with*
	 * operations it still receives: a blocked author's arrive and are thrown
	 * away, and a member removed from a group goes on receiving everything
	 * written in it while showing none of it. Every rule above is what the app
	 * does today, verified in crates/dashchat-node/tests —
	 * `test_blocked_contact_still_receives_the_blockers_messages`,
	 * `test_unblocking_brings_back_what_was_written_during_the_block`,
	 * `test_a_rename_made_during_a_block_never_lands_but_the_next_one_does`
	 * and `test_removed_member_still_receives_what_is_written_after`. Once the
	 * keys are rotated on removal and a block ends the exchange itself,
	 * nothing of either reaches the device: this must then stop delivering to
	 * a removed member (see [`subscriptions`]), and the difference between a
	 * message and a rename disappears with the exchange that carried it.
	 */
	private rejects(holder: string, op: Op): boolean {
		const dropped = this.discarded.get(holder);
		if (dropped?.has(op.id) === true) return true;
		// Sync writes everything to the raw store whatever the blocklist says,
		// and the chat filters what it shows by [`hide`]. Two kinds are turned
		// away as they arrive instead, and never offered again: a profile, so
		// the blocker keeps the name it had, and an invitation to a group,
		// which is an ordinary chat payload — `enforce_blocklist` admits only
		// the control and info of a group from a blocked author, to keep one
		// already joined working, and an invitation is neither.
		if (
			(op.kind !== 'profile' && op.kind !== 'group') ||
			!this.blocks(holder, authorOf(op))
		) {
			return false;
		}
		if (dropped === undefined) this.discarded.set(holder, new Set([op.id]));
		else dropped.add(op.id);
		return true;
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
		// An agent syncs the inbox of every peer it has added, so it sees the
		// invites sent to them too; only the one it was addressed to joins.
		if (op.kind === 'group') return op.member === name ? op.chat : null;
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
 * contacts, no hub anywhere, everyone foregrounded and off the air, and the
 * cloud as reachable as the run found it. */
export function newModel(real: {
	agents: {
		agent: { platform: string; p2p: boolean };
		name: string;
		notificationTexts: NotificationTexts | null;
	}[];
	networks: { ssid: string; home: boolean }[];
	cloudUsable: boolean;
	cloudDegradable: boolean;
	push: boolean;
}): ExpectedModel {
	return new ExpectedModel(
		real.agents.map(({ agent, name, notificationTexts }) => ({
			name,
			mobile: agent.platform !== 'desktop',
			p2p: agent.p2p,
			notifications: notificationTexts,
		})),
		real.networks.map(n => ({ name: n.ssid, home: n.home })),
		real.cloudUsable,
		real.cloudDegradable,
		real.push,
	);
}
