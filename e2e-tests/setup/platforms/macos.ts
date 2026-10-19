import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { waitForPortListening } from '../wait-for-port';
import {
	APP_BINARY,
	agentDir,
	agentPort,
	connectDesktopAgent,
	prepareDesktopAgent,
} from './desktop';

/** A desktop agent on `slot` whose Local Network access stays undetermined,
 *  behind the system alert, until `answerLocalNetworkAlert` answers it. */
export async function launchDesktopAgentWithoutLocalNetworkAccess(
	slot: number,
): Promise<WebdriverIO.Browser> {
	prepareDesktopAgent(slot);
	await openAsNewMacApp(slot);
	return await connectDesktopAgent(slot);
}

/** Launch the e2e build as a macOS app of its own, under a bundle id macOS
 *  has never seen, so that Local Network privacy treats it as a first install.
 *  A process the harness spawns runs on the harness's own access instead.
 *  Every launch leaves a row in System Settings › Local Network. */
async function openAsNewMacApp(slot: number): Promise<void> {
	const port = agentPort(slot);
	const bundle = path.join(agentDir(slot), 'DashChatE2E.app');
	mkdirSync(path.join(bundle, 'Contents', 'MacOS'), { recursive: true });
	const binary = path.join(bundle, 'Contents', 'MacOS', 'dash-chat');
	writeFileSync(binary, withFreshMachOUuid(readFileSync(APP_BINARY)), {
		mode: 0o755,
	});
	writeFileSync(
		path.join(bundle, 'Contents', 'Info.plist'),
		appBundleInfoPlist(`studio.darksoil.dashchat.e2e-${Date.now()}`),
	);
	execFileSync('codesign', ['--force', '--sign', '-', bundle]);
	const env: Record<string, string | undefined> = {
		DATA_DIR: agentDir(slot),
		MAILBOX_URL: process.env.MAILBOX_URL,
		PUSH_NOTIFICATIONS_SERVER_URL: process.env.PUSH_NOTIFICATIONS_SERVER_URL,
		TAURI_WEBDRIVER_PORT: String(port),
	};
	const envArgs = Object.entries(env).flatMap(([key, value]) =>
		value === undefined ? [] : ['--env', `${key}=${value}`],
	);
	execFileSync('open', ['-n', ...envArgs, bundle]);
	await waitForPortListening(port);
}

/** `binary` under a new Mach-O UUID. macOS remembers a Local Network answer
 *  by the executable's UUID too, so a build an earlier run allowed would start
 *  allowed under any bundle id. */
function withFreshMachOUuid(binary: Buffer): Buffer {
	const LC_UUID = 0x1b;
	const commandCount = binary.readUInt32LE(16);
	let offset = 32; // mach_header_64
	for (let i = 0; i < commandCount; i++) {
		const command = binary.readUInt32LE(offset);
		if (command === LC_UUID) {
			randomBytes(16).copy(binary, offset + 8);
			return binary;
		}
		offset += binary.readUInt32LE(offset + 4);
	}
	throw new Error('the e2e build has no LC_UUID load command');
}

function appBundleInfoPlist(bundleId: string): string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>${bundleId}</string>
<key>CFBundleName</key><string>Dash Chat E2E</string>
<key>CFBundleExecutable</key><string>dash-chat</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSLocalNetworkUsageDescription</key><string>Dash Chat e2e run</string>
<key>NSBonjourServices</key><array><string>_dashchat._tcp</string></array>
</dict></plist>
`;
}

/** Whether the harness can click system alerts on macOS: the terminal running
 *  it has Accessibility access. */
export function canAnswerSystemAlerts(): boolean {
	try {
		return (
			execFileSync(
				'osascript',
				['-e', 'tell application "System Events" to get UI elements enabled'],
				{ encoding: 'utf8' },
			).trim() === 'true'
		);
	} catch {
		return false;
	}
}

/** Answer the macOS alert asking whether an app may access the local network.
 *  Needs [`canAnswerSystemAlerts`]. */
export function answerLocalNetworkAlert(button: 'Allow' | 'Don’t Allow'): void {
	execFileSync('osascript', [
		'-e',
		`tell application "System Events" to tell process "UserNotificationCenter" to click button "${button}" of window 1`,
	]);
}
