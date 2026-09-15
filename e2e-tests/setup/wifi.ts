/** What every platform's Wi-Fi control has in common: the shape of a
 *  reading, and the wait for one. */

/** Comfortably longer than a WPA2 association plus DHCP on a busy 2.4GHz AP. */
export const WIFI_REASSOCIATE_MS = 90_000;

export interface WifiInfo {
	/** The SSID the device is associated with, or '' while it is on none. */
	ssid: string;
	/** The device's IPv4 address on its Wi-Fi interface, or '' while it has
	 *  none. */
	address: string;
}

/** How often the Wi-Fi state is re-read while waiting for it: callers time
 *  discovery from the moment the address appears, so the poll has to be fine
 *  next to the budget they hold it to. */
const WIFI_POLL_MS = 250;

/** Poll `read` until it answers non-empty, or throw `timeoutMsg` once
 *  WIFI_REASSOCIATE_MS have passed. */
export async function waitForWifi(
	read: () => string | Promise<string>,
	timeoutMsg: string,
): Promise<string> {
	const deadline = Date.now() + WIFI_REASSOCIATE_MS;
	for (;;) {
		const value = await read();
		if (value !== '') return value;
		if (Date.now() > deadline) throw new Error(timeoutMsg);
		await new Promise(resolve => setTimeout(resolve, WIFI_POLL_MS));
	}
}
