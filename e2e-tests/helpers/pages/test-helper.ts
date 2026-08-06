import type { ChainablePromiseElement } from 'webdriverio';

import type {
	FilePickerRequest,
	TestFileSpec,
} from '../../../ui/tests/setup-utils';

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

	/** Whether the app is rendering its mobile UI. Read from the user agent
	 * because that is exactly what the app's own `isMobile` branches on, so a
	 * test gesture can never drift from the UI that is actually mounted. */
	protected isMobileBuild(): Promise<boolean> {
		return this.agent.execute(() =>
			/iPhone|iPad|iPod|Android/i.test(navigator.userAgent),
		);
	}

	/**
	 * Click `trigger` and answer the file input it opens with `files`, or back
	 * out of that input when there are none, reporting what it asked the OS for.
	 * No native picker or camera appears.
	 */
	protected async answerFilePicker(
		trigger: ChainablePromiseElement,
		files: TestFileSpec[] = [],
	): Promise<FilePickerRequest> {
		await this.agent.execute(
			(specs: TestFileSpec[]) => window.__test.interceptFilePickers(specs),
			files,
		);
		let failure: unknown;
		try {
			await trigger.click();
		} catch (error) {
			failure = error;
		}
		const requests = await this.agent.execute(() =>
			window.__test.collectFilePickers(),
		);
		if (failure) throw failure;
		expect(requests).toHaveLength(1);
		return requests[0];
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
