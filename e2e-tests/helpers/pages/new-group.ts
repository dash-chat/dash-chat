import type { Agent } from '../setup-agents';

export class AddMembersStep {
	constructor(private readonly agent: Agent) {}

	async ready(): Promise<AddMembersStep> {
		await this.agent.waitFor('[data-testid="new-group-members-navbar"]');
		return this;
	}

	clickNext(): Promise<void> {
		return this.agent.clickNewGroupNext();
	}

	async addContactByName(name: string): Promise<void> {
		await this.agent.execute((contactName: string) => {
			const listItems = Array.from(document.querySelectorAll('li'));
			const contactItem = listItems.find(item => {
				const title = item.querySelector('.item-title');
				return title?.textContent?.trim() === contactName;
			}) as HTMLElement | undefined;

			if (!contactItem) {
				throw new Error(
					`Contact "${contactName}" not found in group members list`,
				);
			}

			const clickTarget =
				(contactItem.querySelector('.item-content') as HTMLElement | null) ??
				contactItem;
			clickTarget.click();
		}, name);

		await this.clickNext();
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
