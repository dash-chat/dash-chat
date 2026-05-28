import type { Agent } from '../setup-agents';

export class AddMembersStep {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<AddMembersStep> {
		await this.agent.waitForText('body', 'Adding members not yet implemented');
		return this;
	}

	clickNext(): Promise<void> {
		return this.agent.clickNewGroupNext();
	}
}

export class GroupInfoStep {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<GroupInfoStep> {
		await this.agent.waitFor('[data-testid="new-group-info-back"]');
		return this;
	}

	clickCreate(): Promise<void> {
		return this.agent.clickNewGroupCreate();
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

	onGroupInfoStep(): GroupInfoStep {
		return new GroupInfoStep(this.agent);
	}
}
