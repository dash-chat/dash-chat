/**
 * Recent-photos strip E2E — taps photos from the composer's recent-photos strip
 * to stage them, and sends. The native photo library is
 * unavailable in the harness, so the strip is fed fake photos via the
 * `window.__test.recentPhotos` seam. The strip is mobile-only UI ({#if isMobile}
 * on the media panel), so the test skips on desktop user agents where the
 * attach button opens the MediaMenu instead.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Recent photos strip', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
		await agent1.createProfilePage.createProfile('Alice', 'Recents');
		await agent2.createProfilePage.createProfile('Bob', 'Recents');
		await exchangeContacts(agent1, agent2);
	});

	it('stages and sends photos tapped from the strip', async function () {
		const { composer } = agent1.directChatPage;
		await composer.recentPhotos.injectPhotos(3);

		const opened = await composer.openMediaPanel();
		if (!opened) {
			// Desktop user agent: the attach button opens the MediaMenu, not the
			// mobile media panel that hosts the strip. Covered on-device instead.
			this.skip();
		}

		await composer.recentPhotos.tile(0).click();
		await composer.expectStagedPhotoCount(1);
		await composer.recentPhotos.tile(1).click();
		await composer.expectStagedPhotoCount(2);

		await composer.type('from recents');
		await composer.send();
		await agent1.directChatPage.messages.waitForMessage('from recents');
		await agent2.directChatPage.messages.waitForMessage('from recents');
	});
});
