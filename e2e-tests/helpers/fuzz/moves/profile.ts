/** Changing the name you go by. It reaches everyone that syncs your
 *  announcements, so every device that hears it starts calling you that —
 *  in its chat list, in its pickers and on its notifications — while one that
 *  has not still uses the name it was given before. */
import { type Real, backToChatList, log } from '../agents';
import type { ExpectedModel } from '../model';
import { ActorMove, type Move, type Moves } from './move';

class UpdateProfileMove extends ActorMove {
	actors(m: Readonly<ExpectedModel>): string[] {
		return m.activeNames();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = this.actor(m, real);
		const name = m.nextProfileName();
		log(`${actor.name}: ${this.toString()} -> calls itself ${name}`);
		await backToChatList(actor, m);
		const { homePage, settingsPage, profilePage, editNamePage } = actor.agent;
		await homePage.settingsLink.click();
		await settingsPage.ready();
		await settingsPage.profileLink.click();
		await profilePage.ready();
		await profilePage.editName.click();
		await editNamePage.ready();
		await editNamePage.setName(name);
		await editNamePage.save();
		await profilePage.ready();
		await profilePage.back.click();
		await settingsPage.ready();
		await settingsPage.back.click();
		await homePage.ready();
		m.updateProfile(actor.name, name);
	}

	toString(): string {
		return `updateProfile(${this.agentIdx})`;
	}
}

export const profileMoves: Moves = [
	{ build: a => new UpdateProfileMove(a), weight: 1 },
];

/** The profile moves by the names a search prints them under. */
export const move = {
	updateProfile: (agentIdx: number): Move => new UpdateProfileMove(agentIdx),
};
