/** Screenshots of what an agent showed when something failed, kept under
 *  `.dbs/e2e/failures/` for the flakes and freezes the log alone cannot
 *  explain. */
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** The directory failure screenshots go in, created if need be. */
export function failuresDir(): string {
	const dir = path.join(ROOT, '.dbs', 'e2e', 'failures');
	mkdirSync(dir, { recursive: true });
	return dir;
}

/** `text` as a file-name-safe slug. */
export function failureSlug(text: string): string {
	return text.replace(/[^a-zA-Z0-9]+/g, '-').slice(0, 80);
}

/** Save `agent`'s screen to `file`. A dead session is not an error: the
 *  screenshot is evidence, never the failure itself. */
export async function saveFailureScreenshot(
	agent: WebdriverIO.Browser,
	file: string,
): Promise<void> {
	try {
		// Mobile: screenshot from the native context. A webview-context
		// screenshot goes through chromedriver, which blocks for minutes
		// against the frozen renderer of a backgrounded app — precisely the
		// state many failures leave the device in. The native screenshot
		// always works and also captures system UI like the shade.
		let restoreTo: string | undefined;
		if (agent.isMobile) {
			const context = await agent.getContext();
			if (typeof context === 'string' && context !== 'NATIVE_APP') {
				restoreTo = context;
				await agent.switchContext('NATIVE_APP');
			}
		}
		await agent.saveScreenshot(file);
		if (restoreTo !== undefined) await agent.switchContext(restoreTo);
	} catch {
		/* session may already be dead */
	}
}
