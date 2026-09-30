import { mkdirSync, rmSync } from 'node:fs';
import { remote } from 'webdriverio';

import { UI_TIMEOUT } from '../helpers/timeouts';
import { allocateFreePort } from './allocate-port';
import { killAndWait } from './cleanup';
import { APP_BINARY, e2eDataDir, launchDesktopApp } from './platforms/desktop';
import {
	type Agent,
	makeAgent,
	tapWebElementsAtTheirRect,
} from './setup-agents';
import { waitForTestUtils } from './webview';

export interface EphemeralAgent {
	agent: Agent;
	/** Kill the app and delete everything it stored. */
	kill(): Promise<void>;
}

/** Slots past any `PLATFORMS` could launch, so nothing keyed by slot collides
 *  with a session agent's. */
const EPHEMERAL_SLOT_BASE = 1_000;
let launchedCount = 0;

/**
 * Launch one more desktop agent while a spec runs, on a fresh data dir, beyond
 * the fixed set `PLATFORMS` starts the session with. Only what a spec does
 * through its pages works on it: the slot-keyed helpers (`readLog`,
 * `stopApp`, `restart`, ...) point at directories it doesn't have.
 */
export async function launchEphemeralAgent(): Promise<EphemeralAgent> {
	launchedCount++;
	const slot = EPHEMERAL_SLOT_BASE + launchedCount;
	const dataDir = e2eDataDir(`ephemeral-${launchedCount}`);
	rmSync(dataDir, { recursive: true, force: true });
	mkdirSync(dataDir, { recursive: true });

	const port = await allocateFreePort();
	const app = await launchDesktopApp(APP_BINARY, dataDir, port, {});
	let b: WebdriverIO.Browser;
	let agent: Agent;
	try {
		b = await remote({
			hostname: '127.0.0.1',
			port,
			capabilities: {
				platformName: process.platform === 'darwin' ? 'mac' : 'linux',
			} as WebdriverIO.Capabilities,
			logLevel: 'warn',
			waitforTimeout: UI_TIMEOUT,
		});
		await waitForTestUtils(b);
		tapWebElementsAtTheirRect(b, 'mouse');

		agent = makeAgent(b, slot);
		agent.platform = 'desktop';
		agent.p2p = true;
		await agent.setWideScreen(false);
	} catch (e) {
		// A leftover app would keep advertising over mDNS and skew the rest of the run.
		await killAndWait(app);
		rmSync(dataDir, { recursive: true, force: true });
		throw e;
	}

	return {
		agent,
		async kill() {
			// The WebDriver server lives in the app, so the session may already be gone.
			await b.deleteSession().catch(() => {});
			await killAndWait(app);
			rmSync(dataDir, { recursive: true, force: true });
		},
	};
}
