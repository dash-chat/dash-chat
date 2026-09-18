/** Moves a user makes on what their device is showing: tapping a
 *  notification to get to the chat it came from. */
import {
	type Real,
	type StressAgent,
	at,
	byName,
	log,
	notificationsOf,
} from '../agents';
import type { ExpectedChat, ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** What a tap gets to put the chat on screen: the app may be resuming, or
 *  cold-starting from a force-quit, which on a phone takes seconds. */
const TAP_TIMEOUT = 60_000;

/** Taps one of the notifications a device is showing. The chat it lands on
 * is the property: the route the notification carries must be the chat it
 * came from, whether the app was on another chat, backgrounded or quit. */
class TapNotificationMove extends Move {
	constructor(
		readonly agentIdx: number,
		readonly chatIdx: number,
	) {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.notifiedNames().some(name => tappable(m, name).length > 0);
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		const actor = byName(
			real,
			at(
				m.notifiedNames().filter(name => tappable(m, name).length > 0),
				this.agentIdx,
			),
		);
		const notification = at(tappable(m, actor.name), this.chatIdx);
		const chat = notification.opens;
		if (chat === null) throw new Error('drew a notification with no chat');
		log(`${actor.name}: ${this.toString()} -> ${notification.shows[0]}'s`);
		const notifications = notificationsOf(actor);
		await notifications.tapNotification(notification.shows[0]);
		await notifications.returnToApp();
		// The tap resumes or cold-starts the app onto the chat, which is also
		// where the app clears what it had posted for it.
		m.startApp(actor.name);
		m.foreground(actor.name);
		m.openedChat(actor.name, chat);
		await expectOpened(actor, chat, m);
	}

	toString(): string {
		return `tapNotification(${this.agentIdx},${this.chatIdx})`;
	}
}

/** The notifications a tap can be aimed at and checked: one whose chat the
 *  model can name — a request from someone the agent has not added back opens
 *  one it cannot — and which says something a tap can find it by, which one
 *  built before the sender's profile arrived does not. */
function tappable(m: Readonly<ExpectedModel>, name: string) {
	return m
		.expectedNotifications(name)
		.filter(n => n.opens !== null && n.shows.length > 0);
}

/** Wait until `chat` is on screen, naming whatever else the tap opened when
 *  it is not. */
async function expectOpened(
	actor: StressAgent,
	chat: ExpectedChat,
	m: ExpectedModel,
): Promise<void> {
	const expected = m.chatListName(chat, actor.name);
	const { directChatPage, groupChatPage } = actor.agent;
	const title =
		chat.kind === 'direct' ? directChatPage.peerName : groupChatPage.headerName;
	try {
		await actor.agent.waitUntil(
			async () =>
				(await title.isExisting()) &&
				(await title.getText()).includes(expected),
			{ timeout: TAP_TIMEOUT },
		);
	} catch {
		throw new Error(
			`${actor.name}: tapping ${expected}'s notification opened ` +
				`${await onScreen(actor)}, not their chat`,
		);
	}
}

/** What the app is showing, for a failure to name. */
async function onScreen(actor: StressAgent): Promise<string> {
	const { directChatPage, groupChatPage } = actor.agent;
	if (await directChatPage.peerName.isExisting()) {
		return `${await directChatPage.peerName.getText()}'s chat`;
	}
	if (await groupChatPage.headerName.isExisting()) {
		return `the group "${await groupChatPage.headerName.getText()}"`;
	}
	return 'no chat at all';
}

/** Weighted as heavily as sending: a user whose device is showing a
 *  notification taps one about as readily as they write a message, and every
 *  other move drawn while one is pending takes the opportunity away. */
export const notificationMoves: Moves = [
	{ build: (a, c) => new TapNotificationMove(a, c), weight: 10 },
];

/** The notification moves by the names a search prints them under. */
export const move = {
	tapNotification: (agentIdx: number, chatIdx: number): Move =>
		new TapNotificationMove(agentIdx, chatIdx),
};
