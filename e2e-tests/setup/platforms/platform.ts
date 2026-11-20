import {
	assertHostStillHolds,
	lanHostAddress,
	nonInternalIpv4s,
} from '../host-ip';

/**
 * The mailbox and push-server urls a mobile build bakes in when the run is
 * against a remote deployment (`mailboxPort` is null). Both come from the same
 * env — `just` loads them from `.env.<ENV>` — and they must be a matched pair:
 * the mailbox notifies its own push server, and the device registers its token
 * with the one it was built for. Mobile needs this because those apps bake the
 * urls at build time; desktop just inherits the process env.
 */
export function remoteBakedEnv(): Record<string, string> {
	const mailboxUrl = process.env.MAILBOX_URL;
	if (mailboxUrl === undefined || mailboxUrl === '') {
		throw new Error(
			'Mobile agents need a mailbox: run with a local one, or set ' +
				'MAILBOX_URL to a remote deployment',
		);
	}
	const bakedEnv: Record<string, string> = { MAILBOX_URL: mailboxUrl };
	const pushUrl = process.env.PUSH_NOTIFICATIONS_SERVER_URL;
	if (pushUrl !== undefined && pushUrl !== '') {
		bakedEnv.PUSH_NOTIFICATIONS_SERVER_URL = pushUrl;
	}
	console.log(
		`baking remote mailbox ${mailboxUrl}` +
			(pushUrl ? ` and push server ${pushUrl}` : ''),
	);
	return bakedEnv;
}

/**
 * The mailbox and push-server urls a mobile build bakes in when the run
 * spawned them: at the host's LAN address, so a phone reaches them over the
 * network it is on and loses them with it, on every platform alike. Baked into
 * the build, so picking a wrong-but-plausible address costs a whole run: every
 * cross-device assertion times out with the app quietly showing its offline
 * state.
 */
export function lanBakedEnv(
	mailboxPort: number,
	pushPort: number | null,
): Record<string, string> {
	const host = lanHostAddress();
	if (host === null) {
		const seen = nonInternalIpv4s()
			.map(({ name, address }) => `${name}=${address}`)
			.join(', ');
		throw new Error(
			'No private LAN IPv4 for the phones to reach the mailbox on — ' +
				`set E2E_HOST_IP. Non-internal IPv4 seen: ${seen || 'none'}`,
		);
	}
	const bakedEnv: Record<string, string> = {
		MAILBOX_URL: `https://${host.address}:${mailboxPort}`,
	};
	if (pushPort !== null) {
		bakedEnv.PUSH_NOTIFICATIONS_SERVER_URL = `http://${host.address}:${pushPort}`;
	}
	// Every phone platform of a run bakes the same urls.
	if (process.env._WDIO_LAN_BAKED_URLS === undefined) {
		console.log(`[phones] baking mailbox host ${host.address} (${host.name})`);
		// Workers inherit the launcher's env, so this is what they check the
		// phones still reach.
		process.env._WDIO_LAN_BAKED_URLS = Object.values(bakedEnv).join(',');
	}
	return bakedEnv;
}

/** What [`lanBakedEnv`] baked into this run's phone builds; none in a run
 *  against a remote mailbox. */
export function lanBakedUrls(): URL[] {
	const urls = process.env._WDIO_LAN_BAKED_URLS;
	if (urls === undefined) return [];
	return urls.split(',').map(url => new URL(url));
}

export function assertBakedHostStillHolds(): void {
	const [mailbox] = lanBakedUrls();
	if (mailbox !== undefined) {
		assertHostStillHolds(mailbox.hostname, 'mailbox url');
	}
}

export interface PrepareContext {
	/** Port of the local mailbox server, or null when running against a remote one. */
	mailboxPort: number | null;
	/** Port of the local push-notifications server, or null when push testing is
	 * disabled (FCM_SERVICE_ACCOUNT_KEY unset). */
	pushPort: number | null;
}

/**
 * Everything platform-specific about running a phone agent (Android device
 * or iPhone). The unified wdio config delegates each phone slot to its
 * platform and calls the hooks of every platform in use.
 */
export interface AgentPlatform {
	/** Agent slots (1, 2) that run on this platform. */
	readonly slots: number[];
	/** Multiremote entry for the given slot: driver port + capabilities. */
	remoteOptions(slot: number): {
		port: number;
		capabilities: WebdriverIO.Capabilities;
	};
	onPrepare(ctx: PrepareContext): Promise<void>;
	beforeSession(): Promise<void>;
	afterSession(): Promise<void>;
	onComplete(): Promise<void>;
}
