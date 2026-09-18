/** Blocking a contact and letting them back in. A block never leaves the
 *  blocker's own devices — the one blocked goes on writing, unaware — and
 *  what they write while it is on is thrown away as it arrives, so unblocking
 *  brings back only what comes after. */
import { blockAgent } from '../../flows/block-agent';
import { type Real, at, log } from '../agents';
import { openChat } from '../checks';
import type { ExpectedModel } from '../model';
import { ActorMove, type Move, type Moves } from './move';

class BlockMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly peerIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.blockablePeers(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const peer = at(m.blockablePeers(actor.name), this.peerIdx);
		log(`${actor.name}: ${this.toString()} -> blocks ${peer}`);
		const chat = m.directChat(actor.name, peer);
		await openChat(actor, chat, m);
		await blockAgent(actor.agent);
		m.blockContact(actor.name, peer);
		m.openedChat(actor.name, chat);
	}

	toString(): string {
		return `block(${this.agentIdx},${this.peerIdx})`;
	}
}

class UnblockMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly peerIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.blockedPeers(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const peer = at(m.blockedPeers(actor.name), this.peerIdx);
		log(`${actor.name}: ${this.toString()} -> unblocks ${peer}`);
		const chat = m.directChat(actor.name, peer);
		await openChat(actor, chat, m);
		const { directChatPage } = actor.agent;
		await directChatPage.unblockButton.click();
		await directChatPage.unblockConfirm.waitForClickable();
		await directChatPage.unblockConfirm.click();
		await directChatPage.blockedBanner.waitForDisplayed({ reverse: true });
		m.unblockContact(actor.name, peer);
		m.openedChat(actor.name, chat);
	}

	toString(): string {
		return `unblock(${this.agentIdx},${this.peerIdx})`;
	}
}

export const blockMoves: Moves = [
	{ build: (a, p) => new BlockMove(a, p), weight: 1 },
	{ build: (a, p) => new UnblockMove(a, p), weight: 1 },
];

/** The block moves by the names a search prints them under. */
export const move = {
	block: (agentIdx: number, peerIdx: number): Move =>
		new BlockMove(agentIdx, peerIdx),
	unblock: (agentIdx: number, peerIdx: number): Move =>
		new UnblockMove(agentIdx, peerIdx),
};
