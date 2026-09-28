/**
 * A photo already on screen stays there when the message before it is deleted
 * (#607: "images are not visible after being shown"). Deleting the message
 * that starts the photo's group changes which message starts it; the photo
 * must not be mounted again, which would leave it off screen until its bytes
 * are read back from the node.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO = 'after-deleted-message';

describe('Photo already on screen', () => {
	let agent: Agent;

	before(async function () {
		[agent] = await setupAgents(this, [{ platform: 'any' }]);
		await createProfiles({ Alice: agent });
		await createGroup(agent, 'Solo Group', []);
	});

	it('stays when the message before it is deleted', async () => {
		const { composer, messages } = agent.groupChatPage;
		await composer.sendMessage('about to be deleted');
		await composer.attachNoisePhoto(PHOTO, 1920, 1440);
		await composer.type('the photo');
		await composer.send();
		await messages.waitForPhotoMessage(PHOTO);
		const toDelete = await messages.waitForMessage('about to be deleted');

		const token = await messages.recordPhotoVisibility(PHOTO);
		await toDelete.deleteForEveryone();
		await toDelete.waitForDeleted(await agent.tr('youDeletedThisMessage'));

		expect(await messages.photoHiddenPeriods(token)).toEqual([]);
	});
});
