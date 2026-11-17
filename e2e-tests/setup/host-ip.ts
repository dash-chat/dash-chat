import { networkInterfaces } from 'node:os';

/** Whether a device could route to this address over the LAN. 169.254 is
 *  excluded on purpose: a tethered iPhone shows up as a USB network interface
 *  with a self-assigned address in that range, and baking one of those leaves
 *  the host unreachable from every device. */
function isPrivateLanAddress(address: string): boolean {
	const [a, b] = address.split('.').map(Number);
	return a === 10 || a === 192 || (a === 172 && b >= 16 && b <= 31);
}

/** Built-in wifi/ethernet (en0, en1, …) ahead of USB and virtual interfaces,
 *  lowest number first. A tethered iPhone sharing its connection hands out a
 *  private address too, and it must not win over the real LAN. */
function interfaceRank(name: string): number {
	const en = /^en(\d+)$/.exec(name);
	return en === null ? 1000 : Number(en[1]);
}

/** The host's IPv4 on the LAN, which phones reach the host's servers at, or
 *  `null` when it has none. `E2E_HOST_IP` overrides the detection. */
export function lanHostIp(): string | null {
	const override = process.env.E2E_HOST_IP;
	if (override !== undefined && override !== '') return override;
	const usable = Object.entries(networkInterfaces())
		.flatMap(([name, addrs]) => (addrs ?? []).map(addr => ({ name, addr })))
		.filter(({ addr }) => addr.family === 'IPv4' && !addr.internal)
		.filter(({ addr }) => isPrivateLanAddress(addr.address))
		.sort((a, b) => interfaceRank(a.name) - interfaceRank(b.name));
	if (usable.length === 0) return null;
	const { name, addr } = usable[0];
	console.log(`[host-ip] the host's LAN address is ${addr.address} (${name})`);
	return addr.address;
}
