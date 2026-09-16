/** What a platform's Wi-Fi tooling has to do for a run: the operations the
 *  index exports, minus the platform choice. */
export interface HostWifi {
	/** The host's first Wi-Fi device, or null when it has none. */
	wifiDevice(): string | null;
	/** The SSIDs `device` can see after a fresh scan, or null when the
	 *  platform withholds them. */
	visibleNetworks(device: string): string[] | null;
	/** Join `ssid` on `device` and resolve with the IPv4 address obtained on
	 *  it. */
	joinWifi(device: string, ssid: string, passphrase: string): Promise<string>;
	/** Leave `ssid` and resolve once the card is back on its usual network,
	 *  with an address — where every hub is again from then on. */
	leaveWifi(ssid: string): Promise<void>;
}

/** Poll `read` every 500ms until it answers an address, for up to 20s; past
 *  that, run `onTimeout` and throw `timeoutMsg`. */
export async function waitForAddress(
	read: () => string,
	onTimeout: () => void,
	timeoutMsg: string,
): Promise<string> {
	const deadline = Date.now() + 20_000;
	for (;;) {
		const address = read();
		if (address !== '') return address;
		if (Date.now() > deadline) {
			onTimeout();
			throw new Error(timeoutMsg);
		}
		await new Promise(resolve => setTimeout(resolve, 500));
	}
}
