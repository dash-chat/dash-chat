export abstract class TestPage {
	constructor(protected agent: WebdriverIO.Browser) {}

	abstract ready(): Promise<void>;

	protected async typeInto(selector: string, value: string): Promise<void> {
		await this.agent.$(selector).waitForExist();
		await this.agent.execute(
			(sel: string, val: string) => {
				const el = document.querySelector(sel) as
					| HTMLInputElement
					| HTMLTextAreaElement;
				const proto =
					el.tagName === 'TEXTAREA'
						? HTMLTextAreaElement.prototype
						: HTMLInputElement.prototype;
				const setter = Object.getOwnPropertyDescriptor(proto, 'value')!.set!;
				setter.call(el, val);
				el.dispatchEvent(new Event('input', { bubbles: true }));
				el.dispatchEvent(new Event('change', { bubbles: true }));
			},
			selector,
			value,
		);
	}
}
