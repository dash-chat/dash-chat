import { S } from '../../ui/tests/selectors';
import { type Agent, setupAgent } from '../helpers/setup-agents';

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
		await agent1.createProfile('Alice', 'Test');
		await agent2.createProfile('Bob', 'Test');
	});

	it('adds a contact by uploading a QR code image on desktop', async () => {
		await agent1.navigateToAddContact();
		await agent2.navigateToAddContact();

		const contactCode = await agent2.getContactCode();
		if (!contactCode) throw new Error('agent2 contact code missing');

		await agent1.uploadQrCodeImage(contactCode);

		await agent1.waitUntil(
			async () =>
				agent1.execute(
					(sel: string) => document.querySelector(sel) !== null,
					S.directChat.page,
				),
			{
				timeout: 15_000,
				timeoutMsg: 'Direct chat did not open after QR image upload',
			},
		);
	});

	it('adds agent1 as a contact of agent2 via code (reciprocal)', async () => {
		await agent1.goto('/new-message/add-contact');
		await agent1.waitUntil(
			async () => agent1.execute(() => window.__test.getContactCode() !== null),
			{ timeout: 10_000, timeoutMsg: 'agent1 contact code not available' },
		);
		const contactCode = await agent1.getContactCode();
		if (!contactCode) throw new Error('agent1 contact code missing');
		await agent2.addContact(contactCode);
	});

	it('shows an error toast when the uploaded image contains no QR code', async () => {
		await agent1.goto('/new-message/add-contact');
		await agent1.waitUntil(
			async () =>
				agent1.execute(
					(sel: string) => document.querySelector(sel) !== null,
					S.addContact.fileInput,
				),
			{ timeout: 10_000, timeoutMsg: 'add-contact file input not found' },
		);

		const toastMessage = await agent1.execute(async () => {
			const toastPromise = window.__test.captureNextToastMessage();
			await window.__test.uploadEmptyImage();
			return toastPromise;
		});

		const expected = await agent1.tr('errorNoQrCodeInImage');
		expect(toastMessage).toBe(expected);
	});
});
