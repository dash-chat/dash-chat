import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';
import { tid } from '../helpers/selectors';

async function waitForTextContent(
	agent: Agent,
	selector: string,
	text: string,
): Promise<void> {
	await agent.waitUntil(
		async () =>
			agent.execute(
				(sel: string, t: string) => window.__test.hasText(sel, t),
				selector,
				text,
			),
		{ timeout: 15_000 },
	);
}

describe('Waiting-for-profile placeholder', () => {
	let agent1: Agent;
	let agent2: Agent;
	let waitingText: string;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		waitingText = await agent2.tr('waitingForProfile');
	});

	it('shows the placeholder on direct-chat after one-sided contact addition', async () => {
		await navigateToAddContact(agent1);
		const code1 = await agent1.addContactPage.getContactCode();

		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterCode(code1);
		await agent2.directChatPage.ready();

		await waitForTextContent(agent2, tid('direct-chat-peer-header'), waitingText);
		await waitForTextContent(
			agent2,
			tid('direct-chat-settings-link'),
			waitingText,
		);
	});

	it('shows the placeholder on chat-settings', async () => {
		await agent2.directChatPage.settingsLink.click();
		await waitForTextContent(
			agent2,
			tid('chat-settings-peer-header'),
			waitingText,
		);
	});

	it('shows the placeholder on the home chat-list row', async () => {
		await agent2.chatSettingsPage.back.click();
		await agent2.directChatPage.ready();
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();
		await waitForTextContent(agent2, tid('all-chats-row'), waitingText);
	});
});
