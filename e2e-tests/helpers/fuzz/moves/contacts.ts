/** Meeting people: entering someone's contact link, which sends them a
 *  request, and accepting one that arrived. A pair are contacts once each has
 *  the other — by both entering links, or by one entering and the other
 *  accepting. */
import { SYNC_TIMEOUT } from '../../timeouts';
import {
	type Real,
	addContact,
	at,
	backToChatList,
	byName,
	log,
	openChatByTitle,
} from '../agents';
import { openChat } from '../checks';
import type { ExpectedModel } from '../model';
import { ActorMove, type Move, type Moves } from './move';

class AddContactMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly peerIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.notYetAdded(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const peer = byName(real, at(m.notYetAdded(actor.name), this.peerIdx));
		log(`${actor.name}: ${this.toString()} -> adds ${peer.name}`);
		await addContact(actor, peer, m);
		if (!m.isActive(peer.name)) return;
		if (!m.areContacts(actor.name, peer.name)) {
			// The add that only opens a request puts a row on the peer's list
			// all the same, which settle would miss because the model makes no
			// chat for it until the pair complete. Waiting for it is what stops
			// a later move — a cut link, say — from stranding a request the
			// model has already credited the peer with, leaving every
			// `acceptContact` after it asserting against an op that never
			// arrived. Only once the model says it got there: over a link that
			// is down it has not, and the peer is right to show nothing.
			if (!m.pendingRequestsFor(peer.name).includes(actor.name)) return;
			const title = m.displayName(peer.name, actor.name) ?? actor.name;
			log(`${peer.name}: should now see a request from ${title}`);
			await backToChatList(peer, m);
			await peer.agent.homePage
				.chatListItem(title)
				.waitForExist({ timeout: SYNC_TIMEOUT });
			return;
		}
		// The add that completes a pair puts the chat on the peer's screen too,
		// which settle would miss: the peer may have known the actor's profile
		// since its own add, so its knowledge does not grow here.
		const chat = m.directChat(actor.name, peer.name);
		log(`${peer.name}: should now see ${m.chatListName(chat, peer.name)}`);
		await openChat(peer, chat, m);
	}

	toString(): string {
		return `addContact(${this.agentIdx},${this.peerIdx})`;
	}
}

/** Accepts a request someone sent us, from the chat it opened — what a user
 * does instead of going and entering the requester's link themselves. */
class AcceptContactMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly requesterIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.pendingRequestsFor(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const requester = at(m.pendingRequestsFor(actor.name), this.requesterIdx);
		log(`${actor.name}: ${this.toString()} -> accepts ${requester}`);
		await backToChatList(actor, m);
		await openChatByTitle(
			actor,
			m.displayName(actor.name, requester) ?? requester,
		);
		await actor.agent.directChatPage.acceptContactRequest();
		m.recordAccepted(actor.name, requester);
		m.openedDirectChat(actor.name, requester);
		// Accepting is the add that completes the pair, so the requester's
		// chat stops being pending — which settle would miss for the same
		// reason an add does.
		if (!m.isActive(requester)) return;
		const chat = m.directChat(actor.name, requester);
		log(`${requester}: should now see ${m.chatListName(chat, requester)}`);
		await openChat(byName(real, requester), chat, m);
	}

	toString(): string {
		return `acceptContact(${this.agentIdx},${this.requesterIdx})`;
	}
}

export const exchangeContactMoves: Moves = [
	{ build: (a, p) => new AddContactMove(a, p), weight: 10 },
	{ build: (a, r) => new AcceptContactMove(a, r), weight: 6 },
];

/** The contact moves by the names a search prints them under. */
export const move = {
	addContact: (agentIdx: number, peerIdx: number): Move =>
		new AddContactMove(agentIdx, peerIdx),
	acceptContact: (agentIdx: number, requesterIdx: number): Move =>
		new AcceptContactMove(agentIdx, requesterIdx),
};
