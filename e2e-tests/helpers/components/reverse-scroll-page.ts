import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

const SCROLL_BOTTOM_THRESHOLD = 200;

/**
 * Test driver for an instance of `<ReverseScrollPage>`. Pass the
 * `data-testid` that was set on the component — that landed on the
 * column-reverse scroll container and is what we manipulate here.
 */
export class ReverseScrollPage extends TestHelper {
	private readonly scrollSelector: string;
	readonly scroll;

	constructor(
		agent: WebdriverIO.Browser,
		scrollTestId: string,
	) {
		super(agent);
		this.scrollSelector = tid(scrollTestId);
		this.scroll = this.el(this.scrollSelector);
	}

	isAtBottom(): Promise<boolean> {
		return this.agent.execute(
			(sel: string, threshold: number) => {
				const el = document.querySelector(sel) as HTMLElement | null;
				if (!el) throw new Error('isAtBottom: scroll container not found');
				return Math.abs(el.scrollTop) < threshold;
			},
			this.scrollSelector,
			SCROLL_BOTTOM_THRESHOLD,
		);
	}

	overflow(): Promise<number> {
		return this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) return 0;
			return el.scrollHeight - el.clientHeight;
		}, this.scrollSelector);
	}

	async scrollUp(): Promise<void> {
		await this.agent.execute(
			(sel: string, threshold: number) => {
				const el = document.querySelector(sel) as HTMLElement | null;
				if (!el) throw new Error('scrollUp: scroll container not found');
				const max = el.scrollHeight - el.clientHeight;
				if (max <= threshold) {
					throw new Error(
						`scrollUp: not enough overflow (max=${max}); add more content first`,
					);
				}
				const distance = Math.min(max, 600);
				el.scrollTop = -distance;
				if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
				el.dispatchEvent(new Event('scroll'));
			},
			this.scrollSelector,
			SCROLL_BOTTOM_THRESHOLD,
		);
	}

	async scrollToBottom(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) throw new Error('scrollToBottom: scroll container not found');
			el.scrollTop = 0;
			el.dispatchEvent(new Event('scroll'));
		}, this.scrollSelector);
	}

	async scrollToTop(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const el = document.querySelector(sel) as HTMLElement | null;
			if (!el) throw new Error('scrollToTop: scroll container not found');
			const distance = el.scrollHeight - el.clientHeight;
			el.scrollTop = -distance;
			if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
			el.dispatchEvent(new Event('scroll'));
		}, this.scrollSelector);
	}

	/** Inline opacity of the transparent navbar bg element ReverseScrollPage drives. */
	navbarBgOpacity(): Promise<string | null> {
		return this.agent.execute((sel: string) => {
			const scrollEl = document.querySelector(sel);
			const pageEl = scrollEl?.parentElement;
			if (!pageEl) return null;
			const candidates = pageEl.querySelectorAll('.k-navbar > div.absolute');
			const bg = candidates[candidates.length - 1] as HTMLElement | undefined;
			return bg?.style.opacity ?? null;
		}, this.scrollSelector);
	}
}
