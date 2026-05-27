import type { Agent } from '../setup-agents';

export class HomePage {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<HomePage> {
		const el = await this.agent.homeLoaded();
		if (!el) throw new Error('Not on home page');
		return this;
	}

	clickNewMessage(): Promise<void> {
		return this.agent.clickNewMessage();
	}

	async expectChatListToHaveGroupChatWithName(name: string): Promise<void> {
		try {
			await this.agent.waitUntil(() => this.agent.hasChatListItem(name), {
				timeout: 10_000,
				timeoutMsg: `Expected chat list to contain group "${name}"`,
			});
		} catch {
			const items = await this.agent.execute(() => {
				const list = document.querySelector('[data-testid="all-chats-list"]');
				if (!list) return [];
				return Array.from(list.querySelectorAll('a')).map(
					a => a.textContent?.trim() ?? '',
				);
			});
			throw new Error(
				`Expected chat list to contain group "${name}". Found: ${JSON.stringify(items)}`,
			);
		}
	}
}
