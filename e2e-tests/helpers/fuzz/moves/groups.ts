/** Group chats: creating one with some of the contacts the creator has, and
 *  changing who is in it afterwards. Each membership change reaches the member
 *  it names through their inbox, and is the only one their device hears
 *  about. */
import {
	type Real,
	type StressAgent,
	at,
	backToChatList,
	log,
} from '../agents';
import { openChat } from '../checks';
import type { ExpectedChat, ExpectedModel } from '../model';
import { ActorMove, type Move, type Moves } from './move';

class CreateGroupMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly offsetIdx: number,
		readonly countIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.contactsOf(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
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
		await backToChatList(actor, m);
		await agent.homePage.newMessageButton.click();
		await agent.newMessagePage.ready();
		await agent.newMessagePage.newGroup.click();
		await agent.newGroupPage.addMembersStep.ready();
		for (const member of members) {
			await agent.newGroupPage.addMembersStep.addContactByName(
				m.displayName(actor.name, member) ?? member,
			);
		}
		await agent.newGroupPage.addMembersStep.nextButton.click();
		await agent.newGroupPage.groupInfoStep.ready();
		await agent.newGroupPage.groupInfoStep.setName(name);
		await agent.newGroupPage.groupInfoStep.createButton.click();
		await agent.groupChatPage.ready();
		// Creating a group lands on it, which is where the app clears what it
		// had posted for it.
		m.openedChat(actor.name, m.addGroup(actor.name, members, name));
	}

	toString(): string {
		return `createGroup(${this.agentIdx},${this.offsetIdx},${this.countIdx})`;
	}
}

/** The shared shape of changing who is in a group the actor created: open the
 *  group, go into its info, act, and come back to the chat. */
abstract class MembershipMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly groupIdx: number,
		readonly memberIdx: number,
	) {
		super(agentIdx);
	}

	/** The members this move can act on in `chat`, for the agent that
	 *  created it. */
	abstract candidates(
		m: Readonly<ExpectedModel>,
		chat: ExpectedChat,
		name: string,
	): string[];

	/** Change the membership through the group's info page. */
	abstract change(
		actor: StressAgent,
		member: string,
		m: ExpectedModel,
		chat: ExpectedChat,
	): Promise<void>;

	/** Groups `name` created that this move has someone to act on in. */
	private groups(m: Readonly<ExpectedModel>, name: string): ExpectedChat[] {
		return m
			.groupsCreatedBy(name)
			.filter(chat => this.candidates(m, chat, name).length > 0);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => this.groups(m, n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const chat = at(this.groups(m, actor.name), this.groupIdx);
		const member = at(this.candidates(m, chat, actor.name), this.memberIdx);
		log(`${actor.name}: ${this.toString()} -> ${member} in ${chat.name}`);
		await openChat(actor, chat, m);
		await actor.agent.groupChatPage.infoLink.click();
		await actor.agent.groupInfoPage.ready();
		await this.change(actor, member, m, chat);
		await actor.agent.groupInfoPage.back.click();
		await actor.agent.groupChatPage.ready();
		m.openedChat(actor.name, chat);
	}
}

class AddMemberMove extends MembershipMove {
	candidates(
		m: Readonly<ExpectedModel>,
		chat: ExpectedChat,
		name: string,
	): string[] {
		return m.addableTo(chat, name);
	}

	async change(
		actor: StressAgent,
		member: string,
		m: ExpectedModel,
		chat: ExpectedChat,
	): Promise<void> {
		await actor.agent.groupInfoPage.addMembersLink.click();
		await actor.agent.addMembersPage.ready();
		await actor.agent.addMembersPage.addContactByName(
			m.displayName(actor.name, member) ?? member,
		);
		await actor.agent.addMembersPage.addButton.click();
		await actor.agent.groupInfoPage.ready();
		m.addGroupMember(chat, actor.name, member);
	}

	toString(): string {
		return `addMember(${this.agentIdx},${this.groupIdx},${this.memberIdx})`;
	}
}

class RemoveMemberMove extends MembershipMove {
	candidates(
		m: Readonly<ExpectedModel>,
		chat: ExpectedChat,
		name: string,
	): string[] {
		return m.removableFrom(chat, name);
	}

	async change(
		actor: StressAgent,
		member: string,
		m: ExpectedModel,
		chat: ExpectedChat,
	): Promise<void> {
		await actor.agent.groupInfoPage
			.memberItem(m.displayName(actor.name, member) ?? member)
			.click();
		await actor.agent.groupInfoPage.removeMemberButton.click();
		await actor.agent.groupInfoPage.removeMemberConfirmButton.click();
		m.removeGroupMember(chat, actor.name, member);
	}

	toString(): string {
		return `removeMember(${this.agentIdx},${this.groupIdx},${this.memberIdx})`;
	}
}

/** Renaming a group and rewriting what it is about. Only its creator can:
 *  the edit link is an admin's. Every member's chat list follows, which is
 *  what makes a group's name something a view has rather than something it
 *  is. */
class ModifyGroupInfoMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly groupIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.groupsCreatedBy(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const chat = at(m.groupsCreatedBy(actor.name), this.groupIdx);
		const { name, description } = m.nextGroupInfo();
		log(`${actor.name}: ${this.toString()} -> ${chat.id} becomes ${name}`);
		await openChat(actor, chat, m);
		const { groupChatPage, groupInfoPage, groupInfoEditPage } = actor.agent;
		await groupChatPage.infoLink.click();
		await groupInfoPage.ready();
		await groupInfoPage.editLink.click();
		await groupInfoEditPage.ready();
		await groupInfoEditPage.setName(name);
		await groupInfoEditPage.setDescription(description);
		await groupInfoEditPage.save();
		await groupInfoPage.ready();
		m.setGroupInfo(chat, name, description);
		await expectGroupInfo(actor, chat, m);
		await groupInfoPage.back.click();
		await groupChatPage.ready();
		m.openedChat(actor.name, chat);
	}

	toString(): string {
		return `modifyGroupInfo(${this.agentIdx},${this.groupIdx})`;
	}
}

/** Wait until the info page shows what the model says `sa` has for `chat`. */
async function expectGroupInfo(
	sa: StressAgent,
	chat: ExpectedChat,
	m: Readonly<ExpectedModel>,
): Promise<void> {
	const { name, description } = m.groupInfo(chat, sa.name);
	const page = sa.agent.groupInfoPage;
	await sa.agent.waitUntil(
		async () =>
			(await page.name.getText()).includes(name) &&
			(await page.description.getText()).includes(description),
		{
			timeoutMsg:
				`${sa.name}: the info page never showed "${name}" / ` +
				`"${description}" for ${chat.id}`,
		},
	);
}

/** Leaving a group: the chat stays on the leaver's list, read-only, and
 *  nothing more of it ever reaches them. The app refuses to let a group's
 *  last admin go, so a creator can only leave one nobody else is in. */
class LeaveGroupMove extends ActorMove {
	constructor(
		agentIdx: number,
		readonly groupIdx: number,
	) {
		super(agentIdx);
	}

	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames().filter(n => m.leavableGroups(n).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const chat = at(m.leavableGroups(actor.name), this.groupIdx);
		log(`${actor.name}: ${this.toString()} -> leaves ${chat.name}`);
		await openChat(actor, chat, m);
		await actor.agent.groupChatPage.infoLink.click();
		await actor.agent.groupInfoPage.ready();
		await actor.agent.groupInfoPage.leaveButton.click();
		await actor.agent.groupInfoPage.leaveConfirmButton.waitForExist();
		await actor.agent.groupInfoPage.leaveConfirmButton.click();
		await actor.agent.homePage.ready();
		m.leaveGroup(chat, actor.name);
		m.wentHome(actor.name);
	}

	toString(): string {
		return `leaveGroup(${this.agentIdx},${this.groupIdx})`;
	}
}

export const groupMoves: Moves = [
	{ build: (a, o, c) => new CreateGroupMove(a, o, c), weight: 2 },
	{ build: (a, g, mi) => new AddMemberMove(a, g, mi), weight: 2 },
	{ build: (a, g, mi) => new RemoveMemberMove(a, g, mi), weight: 1 },
	{ build: (a, g) => new LeaveGroupMove(a, g), weight: 1 },
	{ build: (a, g) => new ModifyGroupInfoMove(a, g), weight: 2 },
];

/** The group moves by the names a search prints them under. */
export const move = {
	createGroup: (agentIdx: number, offsetIdx: number, countIdx: number): Move =>
		new CreateGroupMove(agentIdx, offsetIdx, countIdx),
	addMember: (agentIdx: number, groupIdx: number, memberIdx: number): Move =>
		new AddMemberMove(agentIdx, groupIdx, memberIdx),
	removeMember: (agentIdx: number, groupIdx: number, memberIdx: number): Move =>
		new RemoveMemberMove(agentIdx, groupIdx, memberIdx),
	leaveGroup: (agentIdx: number, groupIdx: number): Move =>
		new LeaveGroupMove(agentIdx, groupIdx),
	modifyGroupInfo: (agentIdx: number, groupIdx: number): Move =>
		new ModifyGroupInfoMove(agentIdx, groupIdx),
};
