import type { Agent } from '../setup-agents';

export class AddMembersStep {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<AddMembersStep> {
		await this.agent.waitForText('body', 'Adding members not yet implemented');
		return this;
	}
}

export class NewGroupPage {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<NewGroupPage> {
		const el = await this.agent.newGroupLoaded();
		if (!el) throw new Error('Not on new group page');
		return this;
	}

	onAddMembersStep(): AddMembersStep {
		return new AddMembersStep(this.agent);
	}
}
