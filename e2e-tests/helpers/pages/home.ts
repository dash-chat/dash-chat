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
}
