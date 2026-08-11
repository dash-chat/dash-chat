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
