import { type Agent, waitForTestUtils } from '../../setup/setup-agents';
import type { VisitResult } from './visit-all-pages';

export function formatIssues(res: VisitResult): string {
	const lines: string[] = [];
	for (const p of res.pages) {
		const issues: string[] = [];
		if (p.overflow?.length)
			issues.push(...p.overflow.map(o => `  overflow: ${o}`));
		if (p.darkMode?.issues?.length)
			issues.push(...p.darkMode.issues.map(d => `  dark-mode: ${d}`));
		if (issues.length) lines.push(`[${p.page}]\n${issues.join('\n')}`);
	}
	return lines.join('\n');
}

export function assertNoIssues(res: VisitResult): void {
	if (res.summary.totalIssues > 0) {
		throw new Error(
			`Found ${res.summary.totalIssues} issue(s):\n${formatIssues(res)}`,
		);
	}
}

/** Reload to home and disable CSS transitions so static layout checks don't
 *  race against in-flight color/opacity animations. */
export async function reloadToHome(agent: Agent): Promise<void> {
	await agent.execute(() => {
		delete (window as unknown as { __test?: unknown }).__test;
		window.location.href = '/';
	});
	await waitForTestUtils(agent);
	await agent.execute(() => {
		const id = '__e2e-no-transitions';
		if (document.getElementById(id)) return;
		const style = document.createElement('style');
		style.id = id;
		style.textContent =
			'*, *::before, *::after { transition: none !important; animation: none !important; }';
		document.head.appendChild(style);
	});
	await agent.homePage.ready();
}

export async function switchCombo(
	agent: Agent,
	theme: 'material' | 'ios',
	wideScreen: boolean,
	dark?: boolean,
): Promise<void> {
	await reloadToHome(agent);
	await agent.setTheme(theme);
	await agent.setWideScreen(wideScreen);
	if (dark) await agent.setDarkMode(true);
	await agent.homePage.ready();
}
