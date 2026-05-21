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
}
