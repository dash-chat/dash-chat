import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../timeouts';
import { type Real, byName, goHome, log, openChat } from './agents';
import type { ExpectedModel } from './model';

/** Wait until every agent's UI reflects everything in the model it has not
 * yet been seen reflecting: chats in its chat list, and each unverified
 * message's current state (presence, edited text, deletion, reactions). */
export async function verifyConvergence(
	m: ExpectedModel,
	real: Real,
): Promise<void> {
	// A backgrounded agent can't render anything; bring everyone back first —
	// catching up on what arrived meanwhile is part of what gets verified.
	for (const name of m.backgroundedNames()) {
		const sa = byName(real, name);
		log(`${name}: foregrounding for verification`);
		await sa.agent.startApp();
		await sa.agent.homePage.ready();
		m.foreground(name);
	}
	log('verifying convergence…');
	await Promise.all(
		real.agents.map(async sa => {
			for (const chat of m.chatsFor(sa.name)) {
				const pending = chat.messages.filter(msg => !msg.verified);
				if (chat.verified && pending.length === 0) continue;
				try {
					const page = await openChat(sa, chat, m);
					for (const msg of pending) {
						if (msg.deleted) {
							await page.messages.waitForMessageGone(msg.label);
						} else if (msg.kind === 'photo') {
							await page.messages.waitForPhotoMessage(
								msg.label,
								MEDIA_SYNC_TIMEOUT,
							);
						} else {
							const rendered = await page.messages.waitForMessage(
								msg.text,
								SYNC_TIMEOUT,
							);
							for (const emoji of new Set(msg.reactions.values())) {
								await rendered.waitForReaction(emoji);
							}
						}
					}
					await goHome(sa, page);
				} catch (err) {
					throw new Error(
						`${sa.name} never converged on chat ` +
							`"${m.chatListName(chat, sa.name)}": ${String(err)}`,
					);
				}
			}
		}),
	);
	for (const chat of m.chats) {
		chat.verified = true;
		for (const msg of chat.messages) msg.verified = true;
	}
	log('convergence verified');
}
