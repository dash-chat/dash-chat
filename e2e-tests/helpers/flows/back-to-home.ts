import type { Agent } from '../../setup/setup-agents';

/** Leave each agent's open direct chat for the home page, all at once. */
export async function backToHome(agents: Agent[]): Promise<void> {
	await Promise.all(
		agents.map(async agent => {
			await agent.directChatPage.back.click();
			await agent.homePage.ready();
		}),
	);
}
