import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { tid } from '../../helpers/selectors';
import { type Agent, setupAgents } from '../../setup/setup-agents';

async function waitForTextContent(
	agent: Agent,
	selector: string,
	text: string,
): Promise<void> {
	await agent.waitUntil(async () =>
		agent.execute(
			(sel: string, t: string) => window.__test.hasText(sel, t),
			selector,
			text,
		),
	);
}

describe('Mutual contact requests', () => {
	let alice: Agent;
	let bob: Agent;

	before(async function () {
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await Promise.all([
			alice.createProfilePage.createProfile('Alice', 'Test'),
			bob.createProfilePage.createProfile('Bob', 'Test'),
		]);
	});

	it('establishes contact when both peers send requests to each other', async () => {
		await exchangeContacts(alice, bob);

		// Both should land on the direct chat without needing to press Accept.
		await waitForTextContent(alice, tid('direct-chat-peer-header'), 'Bob Test');
		await waitForTextContent(bob, tid('direct-chat-peer-header'), 'Alice Test');

		// The accept banner should not be present for either side: mutual
		// requests are implicitly accepted.
		expect(await alice.directChatPage.isContactRequestBannerVisible()).toBe(
			false,
		);
		expect(await bob.directChatPage.isContactRequestBannerVisible()).toBe(
			false,
		);
	});

	it('discloses both profiles once mutual contact is established', async () => {
		// The peer avatars resolve from placeholder to real profiles once the
		// contact establishment and profile announcements sync over p2p.
		await Promise.all([
			alice.directChatPage.waitForPeerProfile(),
			bob.directChatPage.waitForPeerProfile(),
		]);
	});
});
