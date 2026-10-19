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

const APP_BUNDLE_NAME = 'DashChatE2E';

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
	const bundle = path.join(agentDir(slot), `${APP_BUNDLE_NAME}.app`);
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

/** Give `binary`, a single-architecture 64-bit Mach-O, a new UUID in place.
 *  macOS remembers a Local Network answer by the executable's UUID too, so a
 *  build an earlier run allowed would start allowed under any bundle id. */
function withFreshMachOUuid(binary: Buffer): Buffer {
	const MH_MAGIC_64 = 0xfeedfacf;
	const LC_UUID = 0x1b;
	if (binary.readUInt32LE(0) !== MH_MAGIC_64) {
		throw new Error('the e2e build is not a single-architecture 64-bit Mach-O');
	}
	const commandCount = binary.readUInt32LE(16);
	let offset = 32; // mach_header_64
	for (let i = 0; i < commandCount && offset + 24 <= binary.length; i++) {
		if (binary.readUInt32LE(offset) === LC_UUID) {
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
<key>CFBundleName</key><string>${APP_BUNDLE_NAME}</string>
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

/** Answer every pending macOS alert asking whether the e2e app may access the
 *  local network. macOS names the app by its bundle or its executable, and
 *  every copy's alert reads the same, so each one gets a click; an alert left
 *  by a killed app no longer responds. Needs [`canAnswerSystemAlerts`]. */
export function answerLocalNetworkAlert(button: 'Allow' | 'Don’t Allow'): void {
	const script = `
tell application "System Events" to tell process "UserNotificationCenter"
	repeat with i from (count of windows) to 1 by -1
		set question to value of static text 1 of window i
		if question contains "“${APP_BUNDLE_NAME}” to find devices on local networks" or question contains "“dash-chat” to find devices on local networks" then
			click button "${button}" of window i
		end if
	end repeat
end tell`;
	execFileSync('osascript', ['-e', script]);
}
