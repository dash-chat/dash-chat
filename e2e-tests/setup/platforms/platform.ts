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

export interface PrepareContext {
	/** Port of the local mailbox server, or null when running against a remote one. */
	mailboxPort: number | null;
	/** Port of the local push-notifications server, or null when push testing is
	 * disabled (FCM_SERVICE_ACCOUNT_KEY unset). */
	pushPort: number | null;
}

/**
 * Everything platform-specific about running an agent (desktop binary vs
 * Android device). The unified wdio config delegates each agent slot to its
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
