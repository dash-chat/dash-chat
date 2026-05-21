import { callTestUtil } from '../setup-agents';

export class NewMessagePage {
	constructor(private readonly b: WebdriverIO.Browser) {}

	async ready(): Promise<NewMessagePage> {
		const el = await this.newMessageLoaded();
		if (!el) throw new Error('Not on new message page');
		return this;
	}

	clickNewGroup(): Promise<void> {
		return callTestUtil(this.b, 'clickNewGroup', []) as Promise<void>;
	}

	newMessageLoaded(): Promise<Element | null> {
		return callTestUtil(
			this.b,
			'newMessageLoaded',
			[],
		) as Promise<Element | null>;
	}
}
