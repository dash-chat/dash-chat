let shuttingDown = false;

/** Mark the app as intentionally shutting down (account deletion): in-flight
 * invokes are expected to fail as the node closes underneath them, and none of
 * it is reportable to the user. */
export function setAppShuttingDown(value: boolean): void {
	shuttingDown = value;
}

export function isAppShuttingDown(): boolean {
	return shuttingDown;
}
