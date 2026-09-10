/** Moves a normal user makes: adding a contact, messaging, reacting,
 *  replying, editing, deleting, creating a group. Each drives one behaviour
 *  through one agent's UI — checking the chat it opens against the model on
 *  the way in — records its ops, and returns to the home page. */
import fc from 'fast-check';

import {
	type ChatPage,
	QUICK_EMOJIS,
	type Real,
	type StressAgent,
	addContact,
	at,
	byName,
	goHome,
	log,
} from '../agents';
import { openChat } from '../checks';
import {
	type ExpectedModel,
	type InteractionTarget,
	type MessageKind,
} from '../model';
import { Move, type Moves } from './move';

/** A voice note's rendered duration, the way `formatDuration` prints it. */
function voiceLabel(seconds: number): string {
	const minutes = Math.floor(seconds / 60);
	return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
}

/** Whoever can act on `eligibleFor`, resolved by `agentIdx`. */
function actorAmong(
	m: ExpectedModel,
	real: Real,
	agentIdx: number,
	eligibleFor: (name: string) => boolean,
): StressAgent {
	return byName(real, at(m.activeNames().filter(eligibleFor), agentIdx));
}

class AddContactMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly peerIdx: number,
	) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.notYetAdded(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = actorAmong(
			m,
			real,
			this.agentIdx,
			n => m.notYetAdded(n).length > 0,
		);
		const peer = byName(real, at(m.notYetAdded(actor.name), this.peerIdx));
		log(`${actor.name}: ${this.toString()} -> adds ${peer.name}`);
		await addContact(actor, peer);
		m.recordAdded(actor.name, peer.name);
	}

	toString(): string {
		return `addContact(${this.agentIdx},${this.peerIdx})`;
	}
}

/** The shared shape of sending something into a chat the actor can send in. */
abstract class SendMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {
		super();
	}

	abstract readonly kind: MessageKind;

	/** Put the message on screen and return its label. */
	abstract send(
		actor: StressAgent,
		m: ExpectedModel,
		page: ChatPage,
	): Promise<string>;

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.sendableChatsFor(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = actorAmong(
			m,
			real,
			this.agentIdx,
			n => m.sendableChatsFor(n).length > 0,
		);
		const chat = at(m.sendableChatsFor(actor.name), this.chatIdx);
		log(
			`${actor.name}: ${this.toString()} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		const label = await this.send(actor, m, page);
		m.addMessage(chat, actor.name, this.kind, label);
		await goHome(actor, page);
	}
}

class SendTextMove extends SendMove {
	readonly kind = 'text';

	async send(
		actor: StressAgent,
		m: ExpectedModel,
		page: ChatPage,
	): Promise<string> {
		const label = m.nextLabel(actor.name);
		await page.composer.sendMessage(label);
		return label;
	}

	toString(): string {
		return `sendText(${this.agentIdx},${this.chatIdx})`;
	}
}

class SendPhotoMove extends SendMove {
	readonly kind = 'photo';

	async send(
		actor: StressAgent,
		m: ExpectedModel,
		{ composer, messages }: ChatPage,
	): Promise<string> {
		const label = m.nextLabel(actor.name);
		await composer.attachPhotos(label);
		await composer.send();
		await messages.waitForPhotoMessage(label);
		return label;
	}

	toString(): string {
		return `sendPhoto(${this.agentIdx},${this.chatIdx})`;
	}
}

class SendFileMove extends SendMove {
	readonly kind = 'file';

	async send(
		actor: StressAgent,
		m: ExpectedModel,
		{ composer, messages }: ChatPage,
	): Promise<string> {
		const label = m.nextLabel(actor.name);
		await composer.attachFile(`${label}.txt`);
		await composer.send();
		await messages.waitForFileMessage(label);
		return label;
	}

	toString(): string {
		return `sendFile(${this.agentIdx},${this.chatIdx})`;
	}
}

class SendVoiceMove extends SendMove {
	readonly kind = 'voice';

	async send(
		actor: StressAgent,
		m: ExpectedModel,
		{ composer, messages }: ChatPage,
	): Promise<string> {
		const seconds = m.nextVoiceSeconds();
		const label = voiceLabel(seconds);
		await composer.recordVoiceMessage(seconds * 1_000);
		await composer.send();
		await messages.waitForVoiceMessageOf(label);
		return label;
	}

	toString(): string {
		return `sendVoice(${this.agentIdx},${this.chatIdx})`;
	}
}

class CreateGroupMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly offsetIdx: number,
		readonly countIdx: number,
	) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.contactsOf(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = actorAmong(
			m,
			real,
			this.agentIdx,
			n => m.contactsOf(n).length > 0,
		);
		const contacts = m.contactsOf(actor.name);
		const count = 1 + (this.countIdx % contacts.length);
		const members = Array.from(
			{ length: count },
			(_, i) => contacts[(this.offsetIdx + i) % contacts.length],
		);
		const name = m.nextGroupName();
		log(
			`${actor.name}: ${this.toString()} -> ${name} with ${members.join(',')}`,
		);
		const { agent } = actor;
		await agent.homePage.ready();
		await agent.homePage.newMessageButton.click();
		await agent.newMessagePage.ready();
		await agent.newMessagePage.newGroup.click();
		await agent.newGroupPage.addMembersStep.ready();
		for (const member of members) {
			await agent.newGroupPage.addMembersStep.addContactByName(member);
		}
		await agent.newGroupPage.addMembersStep.nextButton.click();
		await agent.newGroupPage.groupInfoStep.ready();
		await agent.newGroupPage.groupInfoStep.setName(name);
		await agent.newGroupPage.groupInfoStep.createButton.click();
		await agent.groupChatPage.ready();
		m.addGroup(actor.name, members, name);
		await goHome(actor, agent.groupChatPage);
	}

	toString(): string {
		return `createGroup(${this.agentIdx},${this.offsetIdx},${this.countIdx})`;
	}
}

/** The shared shape of acting on one recent message the actor can see. */
abstract class TargetMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {
		super();
	}

	abstract targets(
		m: Readonly<ExpectedModel>,
		name: string,
	): InteractionTarget[];

	abstract act(
		actor: StressAgent,
		target: InteractionTarget,
		m: ExpectedModel,
		page: ChatPage,
	): Promise<void>;

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => this.targets(m, n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = actorAmong(
			m,
			real,
			this.agentIdx,
			n => this.targets(m, n).length > 0,
		);
		const target = at(this.targets(m, actor.name), this.targetIdx);
		const page = await openChat(actor, target.chat, m);
		await this.act(actor, target, m, page);
		await goHome(actor, page);
	}
}

class ReactMove extends TargetMove {
	constructor(
		agentIdx: number,
		targetIdx: number,
		readonly emojiIdx: number,
	) {
		super(agentIdx, targetIdx);
	}

	targets(m: Readonly<ExpectedModel>, name: string): InteractionTarget[] {
		return m.interactionTargets(name);
	}

	async act(
		actor: StressAgent,
		{ message, view }: InteractionTarget,
		m: ExpectedModel,
		page: ChatPage,
	) {
		// Re-reacting with the current emoji toggles the reaction off; always
		// picking a different one keeps the expected state a plain "has emoji".
		const emoji = at(
			QUICK_EMOJIS.filter(e => e !== view.reactions.get(actor.name)),
			this.emojiIdx,
		);
		log(`${actor.name}: ${this.toString()} -> ${emoji} on ${message.label}`);
		const rendered = await page.messages.waitForMessage(view.text);
		await rendered.reactWith(emoji);
		m.recordReaction(message, actor.name, emoji);
	}

	toString(): string {
		return `react(${this.agentIdx},${this.targetIdx},${this.emojiIdx})`;
	}
}

class ReplyMove extends TargetMove {
	targets(m: Readonly<ExpectedModel>, name: string): InteractionTarget[] {
		return m.interactionTargets(name);
	}

	async act(
		actor: StressAgent,
		{ chat, message, view }: InteractionTarget,
		m: ExpectedModel,
		page: ChatPage,
	) {
		const label = m.nextLabel(actor.name);
		log(`${actor.name}: ${this.toString()} -> ${label} to ${message.label}`);
		const rendered = await page.messages.waitForMessage(view.text);
		await rendered.reply(label);
		m.addMessage(chat, actor.name, 'text', label, message.label);
	}

	toString(): string {
		return `reply(${this.agentIdx},${this.targetIdx})`;
	}
}

class EditMove extends TargetMove {
	targets(m: Readonly<ExpectedModel>, name: string): InteractionTarget[] {
		return m.interactionTargets(name, true);
	}

	async act(
		actor: StressAgent,
		{ message, view }: InteractionTarget,
		m: ExpectedModel,
		page: ChatPage,
	) {
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const rendered = await page.messages.waitForMessage(view.text);
		await rendered.edit(view.text, `${message.label} v${message.edits + 1}`);
		m.recordEdit(message);
	}

	toString(): string {
		return `edit(${this.agentIdx},${this.targetIdx})`;
	}
}

class DeleteMove extends TargetMove {
	targets(m: Readonly<ExpectedModel>, name: string): InteractionTarget[] {
		return m.interactionTargets(name, true).filter(t => !t.message.hasReply);
	}

	async act(
		actor: StressAgent,
		{ message, view }: InteractionTarget,
		m: ExpectedModel,
		page: ChatPage,
	) {
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const rendered = await page.messages.waitForMessage(view.text);
		await rendered.deleteForEveryone();
		m.recordDelete(message);
	}

	toString(): string {
		return `delete(${this.agentIdx},${this.targetIdx})`;
	}
}

const pair = fc.tuple(fc.nat(), fc.nat());
const triple = fc.tuple(fc.nat(), fc.nat(), fc.nat());

/** One move a normal user makes, weighted as a day of use is. */
export const userMoves: Moves = [
	{ arbitrary: pair.map(([a, p]) => new AddContactMove(a, p)), weight: 10 },
	{ arbitrary: pair.map(([a, c]) => new SendTextMove(a, c)), weight: 10 },
	{ arbitrary: pair.map(([a, c]) => new SendPhotoMove(a, c)), weight: 3 },
	{ arbitrary: pair.map(([a, c]) => new SendFileMove(a, c)), weight: 2 },
	{ arbitrary: pair.map(([a, c]) => new SendVoiceMove(a, c)), weight: 2 },
	{
		arbitrary: triple.map(([a, o, c]) => new CreateGroupMove(a, o, c)),
		weight: 2,
	},
	{ arbitrary: triple.map(([a, t, e]) => new ReactMove(a, t, e)), weight: 5 },
	{ arbitrary: pair.map(([a, t]) => new ReplyMove(a, t)), weight: 3 },
	{ arbitrary: pair.map(([a, t]) => new EditMove(a, t)), weight: 3 },
	{ arbitrary: pair.map(([a, t]) => new DeleteMove(a, t)), weight: 2 },
];

/** The user moves by the names a search prints them under. */
export const move = {
	addContact: (agentIdx: number, peerIdx: number): Move =>
		new AddContactMove(agentIdx, peerIdx),
	sendText: (agentIdx: number, chatIdx: number): Move =>
		new SendTextMove(agentIdx, chatIdx),
	sendPhoto: (agentIdx: number, chatIdx: number): Move =>
		new SendPhotoMove(agentIdx, chatIdx),
	sendFile: (agentIdx: number, chatIdx: number): Move =>
		new SendFileMove(agentIdx, chatIdx),
	sendVoice: (agentIdx: number, chatIdx: number): Move =>
		new SendVoiceMove(agentIdx, chatIdx),
	createGroup: (agentIdx: number, offsetIdx: number, countIdx: number): Move =>
		new CreateGroupMove(agentIdx, offsetIdx, countIdx),
	react: (agentIdx: number, targetIdx: number, emojiIdx: number): Move =>
		new ReactMove(agentIdx, targetIdx, emojiIdx),
	reply: (agentIdx: number, targetIdx: number): Move =>
		new ReplyMove(agentIdx, targetIdx),
	edit: (agentIdx: number, targetIdx: number): Move =>
		new EditMove(agentIdx, targetIdx),
	delete: (agentIdx: number, targetIdx: number): Move =>
		new DeleteMove(agentIdx, targetIdx),
};
