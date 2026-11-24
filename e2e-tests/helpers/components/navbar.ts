import { TestHelper } from '../pages/test-helper';
import { checkOverflow } from '../review/checks';

/** The navbar of the page rooted at `pageRoot`. Scoped to that page because in
 * wide-screen layout the sidebar panel's navbar comes first in the DOM. */
export class Navbar extends TestHelper {
	constructor(
		agent: WebdriverIO.Browser,
		private pageRoot: string,
	) {
		super(agent);
	}

	checkOverflow(): Promise<string[]> {
		return checkOverflow(this.agent, `${this.pageRoot} .k-navbar`);
	}
}
