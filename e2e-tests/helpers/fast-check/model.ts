/**
 * The expected-state model for random-behavior stress runs — fast-check's
 * `Model`. Commands record what they did in here, and convergence checks wait
 * until every agent's UI reflects it. It holds only names and expected state,
 * never browser handles: `check(model)` must stay pure.
 *
 * With a network capacity, it also tracks the LANs the run has raised and
 * which of them each agent and each hub is on, which is all the connection
 * chip's expected state derives from.
 */

export interface ExpectedMessage {
	/** Unique greppable id, always contained in the rendered text (or photo
	 * filename), so it can be waited for without colliding with any other
	 * message of the run. */
	label: string;
	kind: 'text' | 'photo';
	sender: string;
	/** Current expected text content: the label plus a version suffix once
	 * edited. Photos carry no text; their label matches the filename. */
	text: string;
	deleted: boolean;
	edits: number;
	/** Reactor name → emoji. One reaction per agent; the latest replaces. */
	reactions: Map<string, string>;
	/** Label of the message this one replies to. */
	replyTo?: string;
	/** A replied-to message is never deleted: its text lives on in the reply
	 * quote, which would make "label gone from the chat" unverifiable. */
	hasReply: boolean;
	/** Whether every member has been seen rendering this message's current
	 * state. Reset whenever a command changes it. */
	verified: boolean;
}

export interface ExpectedChat {
	kind: 'direct' | 'group';
	/** Group name; '' for direct chats, which are opened by peer name. */
	name: string;
	members: string[];
	messages: ExpectedMessage[];
	/** Whether every member has seen this chat appear in its chat list. */
	verified: boolean;
}

export interface ExpectedHub {
	name: string;
	/** The LAN it is on, or null while it is down. */
	network: string | null;
}

/** A LAN a run raised, by its real network name. */
export interface ExpectedNetwork {
	name: string;
}

/** How many of a chat's latest messages commands may interact with — recent
 * enough to still be rendered near the bottom without scrolling. */
const RECENT_WINDOW = 8;

export class ExpectedModel {
	readonly chats: ExpectedChat[] = [];
	/** The LANs currently up. */
	readonly networks: ExpectedNetwork[] = [];
	readonly hubs: ExpectedHub[] = [];
	/** One-sided adds already performed, as 'from>to'. A pair are contacts —
	 * and their direct chat enters the model — once both directions exist. */
	private added = new Set<string>();
	/** Agents currently backgrounded. Their UIs cannot be driven, but peers
	 * keep sending to them — the catch-up-on-foreground case under test. */
	private backgrounded = new Set<string>();
	/** Agent name → the LAN it is on; absent while it is on none. */
	private network = new Map<string, string>();
	private messageCounter = 0;
	private groupCounter = 0;
	private hubCounter = 0;
	private networkCounter = 0;

	constructor(
		readonly agents: { name: string; mobile: boolean }[],
		/** How many LANs can be up at once: one per Wi-Fi card the host has.
		 * Zero keeps every network command out of the run. */
		readonly networkCapacity = 0,
	) {}

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

	canCreateNetwork(): boolean {
		return this.networks.length < this.networkCapacity;
	}

	/** Zero-padded so `dash-e2e-net-01` can never substring-match `-010`. */
	createNetwork(): ExpectedNetwork {
		const network: ExpectedNetwork = {
			name: `dash-e2e-net-${String(++this.networkCounter).padStart(2, '0')}`,
		};
		this.networks.push(network);
		return network;
	}

	/** Take a LAN down. Its hubs stay hubs, on no network until they join
	 * another; its agents end up on no network. */
	killNetwork(name: string): void {
		const index = this.networks.findIndex(n => n.name === name);
		if (index === -1) throw new Error(`no network named ${name}`);
		this.networks.splice(index, 1);
		for (const hub of this.hubs) {
			if (hub.network === name) hub.network = null;
		}
		for (const [agent, network] of this.network) {
			if (network === name) this.network.delete(agent);
		}
	}

	/** Zero-padded so `hub-01` can never substring-match `hub-010`. */
	createHub(): ExpectedHub {
		const hub: ExpectedHub = {
			name: `hub-${String(++this.hubCounter).padStart(2, '0')}`,
			network: null,
		};
		this.hubs.push(hub);
		return hub;
	}

	hub(name: string): ExpectedHub {
		const found = this.hubs.find(h => h.name === name);
		if (found === undefined) throw new Error(`no hub named ${name}`);
		return found;
	}

	/** Hubs `name`'s app must show as connected: the ones on its network,
	 * none while it is on no network. */
	expectedHubs(name: string): number {
		const network = this.networkOf(name);
		if (network === null) return 0;
		return this.hubs.filter(h => h.network === network).length;
	}

	/** Whether `a` and `b` can sync with each other right now: both on the
	 * same LAN. A phone off the air, or on another LAN, reaches nobody. */
	reachable(a: string, b: string): boolean {
		const network = this.networkOf(a);
		return network !== null && network === this.networkOf(b);
	}

	isBackgrounded(name: string): boolean {
		return this.backgrounded.has(name);
	}

	areContacts(a: string, b: string): boolean {
		return this.added.has(`${a}>${b}`) && this.added.has(`${b}>${a}`);
	}

	/** The direct chat between `a` and `b`, which exist once they are contacts. */
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
		this.chats.push({
			kind: 'direct',
			name: '',
			members: [from, to],
			messages: [],
			verified: false,
		});
	}

	/** Peers whose add-contact link `name` has not entered yet. */
	notYetAdded(name: string): string[] {
		return this.names().filter(
			other => other !== name && !this.added.has(`${name}>${other}`),
		);
	}

	contactsOf(name: string): string[] {
		return this.names().filter(
			other => other !== name && this.areContacts(name, other),
		);
	}

	/** Chats `name` is a member of. */
	chatsFor(name: string): ExpectedChat[] {
		return this.chats.filter(c => c.members.includes(name));
	}

	addGroup(creator: string, members: string[], name: string): ExpectedChat {
		const chat: ExpectedChat = {
			kind: 'group',
			name,
			members: [creator, ...members],
			messages: [],
			verified: false,
		};
		this.chats.push(chat);
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

	addMessage(
		chat: ExpectedChat,
		sender: string,
		kind: 'text' | 'photo',
		label: string,
		replyTo?: string,
	): ExpectedMessage {
		const message: ExpectedMessage = {
			label,
			kind,
			sender,
			text: label,
			deleted: false,
			edits: 0,
			reactions: new Map(),
			replyTo,
			hasReply: false,
			verified: false,
		};
		chat.messages.push(message);
		return message;
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

	/** Recent live text messages `name` may interact with (react, reply, and —
	 * with `ownOnly` — edit or delete), across its chats. */
	interactionTargets(
		name: string,
		ownOnly = false,
	): { chat: ExpectedChat; message: ExpectedMessage }[] {
		return this.chatsFor(name).flatMap(chat =>
			chat.messages
				.filter(m => m.kind === 'text' && !m.deleted)
				.slice(-RECENT_WINDOW)
				.filter(m => !ownOnly || m.sender === name)
				.map(message => ({ chat, message })),
		);
	}
}

/** The model for `real`, before anything has happened: no chats, no
 * contacts, no LAN up, everyone foregrounded and off the air. */
export function newModel(real: {
	agents: { agent: { platform: string }; name: string }[];
	wifiDevices: string[];
}): ExpectedModel {
	return new ExpectedModel(
		real.agents.map(({ agent, name }) => ({
			name,
			mobile: agent.platform !== 'desktop',
		})),
		real.wifiDevices.length,
	);
}
