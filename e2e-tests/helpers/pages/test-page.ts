export abstract class TestPage {
	constructor(protected agent: WebdriverIO.Browser) {}

	abstract ready(): Promise<void>;
}
