import { tid } from '../selectors';

export abstract class TestPage {
	constructor(protected agent: WebdriverIO.Browser) {}

	abstract ready(): Promise<void>;

	/** A `data-testid` element that re-resolves on every use, so it never reuses
	 * a stale handle across re-renders. Behaves exactly like `$(tid(id))`. */
	protected el(id: string) {
		return new Proxy(this.agent.$(tid(id)), {
			get: (_target, prop) => {
				const fresh = this.agent.$(tid(id));
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
