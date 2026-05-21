import type { Agent } from '../setup-agents';

export class NewMessagePage {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<NewMessagePage> {
		const el = await this.agent.newMessageLoaded();
		if (!el) throw new Error('Not on new message page');
		return this;
	}

	clickNewGroup(): Promise<void> {
		return this.agent.clickNewGroup();
	}
}
