import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('QR code image upload', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
	});

	it('creates profiles on both agents', async () => {
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
	});

	it('adds a contact by uploading a QR code image on desktop', async () => {
		await navigateToAddContact(agent1);
		await navigateToAddContact(agent2);

		const contactCode = await agent2.addContactPage.getAddContactLink();
		await agent1.addContactPage.uploadQrCodeImage(contactCode);

		await agent1.directChatPage.ready();
	});

	it('shows an error toast when the uploaded image contains no QR code', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await navigateToAddContact(agent1);

		await agent1.addContactPage.uploadEmptyImage();
		await agent1.toast.expectMessage(await agent1.tr('errorNoQrCodeInImage'));
	});
});
