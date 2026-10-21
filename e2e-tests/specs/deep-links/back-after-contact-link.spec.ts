import { createProfiles } from '../../helpers/flows/create-profiles';
import { contactLinkOf } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/** Past the 700 ms ProcessLifecycleOwner delay before the app's `on_pause` runs. */
const ON_PAUSE_SETTLE_MS = 3_000;

describe('Back after opening the app from a contact link', function () {
	let phone: Agent;
	let link: string;

	before(async function () {
		let peer: Agent;
		[phone, peer] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'any' },
		]);
		await createProfiles({ Phone: phone, Peer: peer });
		link = await contactLinkOf(peer);
	});

	it('sends the app to the background, and it comes back to the chat', async () => {
		await phone.stopApp();
		await phone.handleDeepLink(link);
		await phone.startApp();
		await phone.directChatPage.ready();

		phone.pressSystemBack();
		await phone.pause(ON_PAUSE_SETTLE_MS);

		expect(await phone.isAppRunning()).toBe(true);
		expect(phone.readLog()).not.toContain('panicked at');

		await phone.startApp();
		await phone.directChatPage.ready();
	});
});
