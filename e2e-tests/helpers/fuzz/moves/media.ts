/** Sending what has bytes behind it: a photo, a file, a voice note. The bytes
 *  travel separately from the message that carries them, so a receiver can
 *  know of one before it can show it. */
import { type ChatPage, type StressAgent, at, log } from '../agents';
import { type ExpectedModel, PHOTO_COUNTS } from '../model';
import { type Move, type Moves } from './move';
import { SendMove } from './send';

/** A voice note's rendered duration, the way `formatDuration` prints it. */
function voiceLabel(seconds: number): string {
	const minutes = Math.floor(seconds / 60);
	return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
}

class SendPhotoMove extends SendMove {
	readonly kind = 'photo';

	constructor(
		agentIdx: number,
		chatIdx: number,
		readonly countIdx: number,
	) {
		super(agentIdx, chatIdx);
	}

	protected photos(): number {
		return at(PHOTO_COUNTS, this.countIdx);
	}

	async send(
		actor: StressAgent,
		m: ExpectedModel,
		{ composer, messages }: ChatPage,
	): Promise<string> {
		const label = m.nextLabel(actor.name);
		// Every photo of a message is staged the same way and shares its name,
		// so one wait finds the message however many it carries.
		for (let i = 0; i < this.photos(); i++) {
			await composer.attachPhotos(label);
		}
		await composer.send();
		await messages.waitForPhotoMessage(label);
		return label;
	}

	toString(): string {
		return `sendPhoto(${this.agentIdx},${this.chatIdx},${this.countIdx})`;
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

export const mediaMoves: Moves = [
	{ build: (a, b, c) => new SendPhotoMove(a, b, c), weight: 3 },
	{ build: (a, c) => new SendFileMove(a, c), weight: 2 },
	{ build: (a, c) => new SendVoiceMove(a, c), weight: 2 },
];

/** The media moves by the names a search prints them under. */
export const move = {
	sendPhoto: (agentIdx: number, chatIdx: number, countIdx: number): Move =>
		new SendPhotoMove(agentIdx, chatIdx, countIdx),
	sendFile: (agentIdx: number, chatIdx: number): Move =>
		new SendFileMove(agentIdx, chatIdx),
	sendVoice: (agentIdx: number, chatIdx: number): Move =>
		new SendVoiceMove(agentIdx, chatIdx),
};
