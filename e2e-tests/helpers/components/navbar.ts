import { TestHelper } from '../pages/test-helper';

export class Navbar extends TestHelper {
	/** Returns descriptions of any navbar overflow issues. */
	checkOverflow(): Promise<string[]> {
		return this.agent.execute(() => {
			const navbar = document.querySelector('.k-navbar');
			if (!navbar) return ['Navbar element not found'];
			const issues: string[] = [];
			if (navbar.scrollWidth > navbar.clientWidth + 2) {
				issues.push('Navbar has horizontal overflow');
			}
			navbar.querySelectorAll('*').forEach(el => {
				const style = window.getComputedStyle(el);
				const clipped =
					style.overflowX === 'hidden' ||
					style.overflowX === 'clip' ||
					style.overflow === 'hidden' ||
					style.overflow === 'clip' ||
					style.textOverflow === 'ellipsis';
				if (clipped) return;
				if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
					const text = el.textContent?.substring(0, 60).trim();
					if (text)
						issues.push(
							`Overflow in navbar <${el.tagName.toLowerCase()}>: "${text}"`,
						);
				}
			});
			return issues.slice(0, 10);
		});
	}
}
