/** The shared shape of sending something into a chat: text and every kind of
 *  media differ only in what they put on screen and what the model records. */
import { type ChatPage, type Real, type StressAgent, at, log } from '../agents';
import { openChat } from '../checks';
import type { ExpectedModel, MessageKind } from '../model';
import { ActorMove } from './move';

export abstract class SendMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly chatIdx: number,
	) {
		super(agentIdx);
	}

	abstract readonly kind: MessageKind;

	/** Photos only: how many the message carries, which is what its
	 *  notification is worded by. */
	protected photos(): number | undefined {
		return undefined;
	}

	/** Put the message on screen and return its label. */
	abstract send(
		actor: StressAgent,
		m: ExpectedModel,
		page: ChatPage,
	): Promise<string>;

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.sendableChatsFor(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const chat = at(m.sendableChatsFor(actor.name), this.chatIdx);
		log(
			`${actor.name}: ${this.toString()} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		const label = await this.send(actor, m, page);
		m.addMessage(chat, actor.name, this.kind, label, undefined, this.photos());
	}
}
