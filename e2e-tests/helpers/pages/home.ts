import { callTestUtil } from '../setup-agents';

export class HomePage {
	constructor(private readonly b: WebdriverIO.Browser) {}

	async ready(): Promise<HomePage> {
		const el = await this.homeLoaded();
		if (!el) throw new Error('Not on home page');
		return this;
	}

	clickNewMessage(): Promise<void> {
		return callTestUtil(this.b, 'clickNewMessage', []) as Promise<void>;
	}

	homeLoaded(): Promise<Element | null> {
		return callTestUtil(this.b, 'homeLoaded', []) as Promise<Element | null>;
	}

	firstChatTooltip(): Promise<Element | null> {
		return callTestUtil(
			this.b,
			'firstChatTooltip',
			[],
		) as Promise<Element | null>;
	}

	getChatListItem(contactName: string): Promise<Element | null> {
		return callTestUtil(this.b, 'getChatListItem', [
			contactName,
		]) as Promise<Element | null>;
	}

	hasChatListItem(contactName: string): Promise<boolean> {
		return callTestUtil(this.b, 'hasChatListItem', [
			contactName,
		]) as Promise<boolean>;
	}

	checkChatListOverflow(): Promise<string[]> {
		return callTestUtil(this.b, 'checkChatListOverflow', []) as Promise<
			string[]
		>;
	}
}
