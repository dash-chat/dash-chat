/**
 * The random "normal user" command pool. Each command drives one behavior a
 * real user would — adding a contact, messaging, reacting, backgrounding the
 * app — through one agent's UI, records the expected outcome in the
 * ExpectedModel, and returns to the home page, so any generated sequence is
 * valid.
 *
 * Commands carry abstract indices (resolved modulo the eligible options at
 * run time), the canonical fast-check pattern for targets that only exist
 * once earlier commands have run.
 */
import fc from 'fast-check';

import { navigateToAddContact } from '../flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../timeouts';
import {
	QUICK_EMOJIS,
	type Real,
	at,
	byName,
	goHome,
	log,
	openChat,
} from './agents';
import { ExpectedModel } from './model';
import { verifyConvergence } from './verify';

type Cmd = fc.AsyncCommand<ExpectedModel, Real>;

class AddContactCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly peerIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.notYetAdded(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.notYetAdded(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const peer = byName(real, at(m.notYetAdded(actor.name), this.peerIdx));
		log(`${actor.name}: ${this.toString()} -> adds ${peer.name}`);
		await navigateToAddContact(actor.agent);
		await actor.agent.addContactPage.enterAddContactLink(peer.link);
		await actor.agent.directChatPage.ready();
		await actor.agent.directChatPage.back.click();
		await actor.agent.homePage.ready();
		m.recordAdded(actor.name, peer.name);
	}

	toString(): string {
		return `addContact(${this.agentIdx},${this.peerIdx})`;
	}
}

class SendTextCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.chatsFor(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.chatsFor(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const chat = at(m.chatsFor(actor.name), this.chatIdx);
		const label = m.nextLabel(actor.name);
		log(
			`${actor.name}: ${this.toString()} -> ${label} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		await page.composer.sendMessage(label);
		m.addMessage(chat, actor.name, 'text', label);
		await goHome(actor, page);
	}

	toString(): string {
		return `sendText(${this.agentIdx},${this.chatIdx})`;
	}
}

class SendPhotoCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.chatsFor(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.chatsFor(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const chat = at(m.chatsFor(actor.name), this.chatIdx);
		const label = m.nextLabel(actor.name);
		log(
			`${actor.name}: ${this.toString()} -> ${label} in ${m.chatListName(chat, actor.name)}`,
		);
		const page = await openChat(actor, chat, m);
		// Direct chats mount the composer only once the chat leaves the pending
		// state, which needs the peer's profile to have synced.
		await page.composer.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		await page.composer.attachPhotos(label);
		await page.composer.send();
		await page.messages.waitForPhotoMessage(label);
		m.addMessage(chat, actor.name, 'photo', label);
		await goHome(actor, page);
	}

	toString(): string {
		return `sendPhoto(${this.agentIdx},${this.chatIdx})`;
	}
}

class CreateGroupCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly offsetIdx: number,
		readonly countIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.contactsOf(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => m.contactsOf(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
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

class ReactCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
		readonly emojiIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name),
			this.targetIdx,
		);
		// Re-reacting with the current emoji toggles the reaction off; always
		// picking a different one keeps the expected state a plain "has emoji".
		const emoji = at(
			QUICK_EMOJIS.filter(e => e !== message.reactions.get(actor.name)),
			this.emojiIdx,
		);
		log(`${actor.name}: ${this.toString()} -> ${emoji} on ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.reactWith(emoji);
		message.reactions.set(actor.name, emoji);
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `react(${this.agentIdx},${this.targetIdx},${this.emojiIdx})`;
	}
}

class ReplyCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name),
			this.targetIdx,
		);
		const label = m.nextLabel(actor.name);
		log(`${actor.name}: ${this.toString()} -> ${label} to ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.reply(label);
		message.hasReply = true;
		m.addMessage(chat, actor.name, 'text', label, message.label);
		await goHome(actor, page);
	}

	toString(): string {
		return `reply(${this.agentIdx},${this.targetIdx})`;
	}
}

class EditCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => m.interactionTargets(n, true).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m
			.activeNames()
			.filter(n => m.interactionTargets(n, true).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(
			m.interactionTargets(actor.name, true),
			this.targetIdx,
		);
		const newText = `${message.label} v${message.edits + 1}`;
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.edit(message.text, newText);
		message.edits += 1;
		message.text = newText;
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `edit(${this.agentIdx},${this.targetIdx})`;
	}
}

class DeleteCommand implements Cmd {
	constructor(
		readonly agentIdx: number,
		readonly targetIdx: number,
	) {}

	private targets(m: Readonly<ExpectedModel>, name: string) {
		return m.interactionTargets(name, true).filter(t => !t.message.hasReply);
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().some(n => this.targets(m, n).length > 0);
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const eligible = m.activeNames().filter(n => this.targets(m, n).length > 0);
		const actor = byName(real, at(eligible, this.agentIdx));
		const { chat, message } = at(this.targets(m, actor.name), this.targetIdx);
		log(`${actor.name}: ${this.toString()} -> ${message.label}`);
		const page = await openChat(actor, chat, m);
		const rendered = await page.messages.waitForMessage(message.text);
		await rendered.deleteForEveryone();
		message.deleted = true;
		message.verified = false;
		await goHome(actor, page);
	}

	toString(): string {
		return `delete(${this.agentIdx},${this.targetIdx})`;
	}
}

/** Backgrounds an agent and leaves it backgrounded: later commands keep
 * acting through the other agents (including sending to this one), and a
 * ForegroundCommand — or the next convergence check — brings it back, making
 * catch-up-after-background part of every run. */
class BackgroundCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeMobileNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.activeMobileNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.backgroundApp();
		m.background(actor.name);
	}

	toString(): string {
		return `background(${this.agentIdx})`;
	}
}

class ForegroundCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.backgroundedNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.backgroundedNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		await actor.agent.startApp();
		await actor.agent.homePage.ready();
		m.foreground(actor.name);
	}

	toString(): string {
		return `foreground(${this.agentIdx})`;
	}
}

class RestartCommand implements Cmd {
	constructor(readonly agentIdx: number) {}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.activeNames().length > 0;
	}

	async run(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(real, at(m.activeNames(), this.agentIdx));
		log(`${actor.name}: ${this.toString()}`);
		if (actor.agent.platform === 'desktop') {
			await actor.agent.restart();
		} else {
			// Not restart(): a new Appium session fast-resets (pm clear) on
			// Android, wiping the profile. Stop + activate keeps the data dir.
			await actor.agent.stopApp();
			await actor.agent.startApp();
		}
		await actor.agent.homePage.ready();
	}

	toString(): string {
		return `restart(${this.agentIdx})`;
	}
}

class CheckpointCommand implements Cmd {
	check(): boolean {
		return true;
	}

	async run(model: ExpectedModel, real: Real): Promise<void> {
		await verifyConvergence(model, real);
	}

	toString(): string {
		return 'checkpoint()';
	}
}

export const commandArbitrary = fc.oneof(
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, p]) => new AddContactCommand(a, p)),
		weight: 10,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, c]) => new SendTextCommand(a, c)),
		weight: 10,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, c]) => new SendPhotoCommand(a, c)),
		weight: 4,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat(), fc.nat())
			.map(([a, o, c]) => new CreateGroupCommand(a, o, c)),
		weight: 2,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat(), fc.nat())
			.map(([a, t, e]) => new ReactCommand(a, t, e)),
		weight: 5,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new ReplyCommand(a, t)),
		weight: 3,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new EditCommand(a, t)),
		weight: 3,
	},
	{
		arbitrary: fc
			.tuple(fc.nat(), fc.nat())
			.map(([a, t]) => new DeleteCommand(a, t)),
		weight: 2,
	},
	{
		arbitrary: fc.nat().map(a => new BackgroundCommand(a)),
		weight: 3,
	},
	{
		arbitrary: fc.nat().map(a => new ForegroundCommand(a)),
		weight: 3,
	},
	{
		arbitrary: fc.nat().map(a => new RestartCommand(a)),
		weight: 1,
	},
	{
		arbitrary: fc.constant(new CheckpointCommand()),
		weight: 2,
	},
);
