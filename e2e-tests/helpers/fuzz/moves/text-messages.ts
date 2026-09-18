/** What a user does with a text message: writing one, and acting on one that
 *  is already there — reacting, replying, editing, deleting. */
import {
	type ChatPage,
	QUICK_EMOJIS,
	type Real,
	type StressAgent,
	at,
	log,
} from '../agents';
import { openChat } from '../checks';
import type { ExpectedModel, InteractionTarget } from '../model';
import { ActorMove, type Move, type Moves } from './move';
import { SendMove } from './send';

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

/** The shared shape of acting on one recent message the actor can see. */
abstract class TargetMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly targetIdx: number,
	) {
		super(agentIdx);
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

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => this.targets(m, n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const target = at(this.targets(m, actor.name), this.targetIdx);
		const page = await openChat(actor, target.chat, m);
		await this.act(actor, target, m, page);
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

export const textMessageMoves: Moves = [
	{ build: (a, c) => new SendTextMove(a, c), weight: 10 },
	{ build: (a, t, e) => new ReactMove(a, t, e), weight: 5 },
	{ build: (a, t) => new ReplyMove(a, t), weight: 3 },
	{ build: (a, t) => new EditMove(a, t), weight: 3 },
	{ build: (a, t) => new DeleteMove(a, t), weight: 2 },
];

/** The text-message moves by the names a search prints them under. */
export const move = {
	sendText: (agentIdx: number, chatIdx: number): Move =>
		new SendTextMove(agentIdx, chatIdx),
	react: (agentIdx: number, targetIdx: number, emojiIdx: number): Move =>
		new ReactMove(agentIdx, targetIdx, emojiIdx),
	reply: (agentIdx: number, targetIdx: number): Move =>
		new ReplyMove(agentIdx, targetIdx),
	edit: (agentIdx: number, targetIdx: number): Move =>
		new EditMove(agentIdx, targetIdx),
	delete: (agentIdx: number, targetIdx: number): Move =>
		new DeleteMove(agentIdx, targetIdx),
};
