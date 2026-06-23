export abstract class TestHelper {
	constructor(protected agent: WebdriverIO.Browser) {}

	/** Wraps `$(selector)` so it re-resolves on every use — never reusing a stale
	 * handle across re-renders. Pass `tid('id')` for a `data-testid` element. */
	protected el(selector: string) {
		return new Proxy(this.agent.$(selector), {
			get: (_target, prop) => {
				const fresh = this.agent.$(selector);
				const value = Reflect.get(fresh, prop);
				return typeof value === 'function' ? value.bind(fresh) : value;
			},
		});
	}

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
