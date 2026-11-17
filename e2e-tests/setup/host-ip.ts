import { networkInterfaces } from 'node:os';

/** An IPv4 address of one of the host's network interfaces. */
export interface HostAddress {
	name: string;
	address: string;
}

/** Every IPv4 address on the host's interfaces but loopback. */
export function nonInternalIpv4s(): HostAddress[] {
	return Object.entries(networkInterfaces()).flatMap(([name, addrs]) =>
		(addrs ?? [])
			.filter(addr => addr.family === 'IPv4' && !addr.internal)
			.map(addr => ({ name, address: addr.address })),
	);
}

/** Whether a device could route to this address over the LAN. 169.254 is
 *  excluded on purpose: a tethered iPhone shows up as a USB network interface
 *  with a self-assigned address in that range, and baking one of those leaves
 *  the host unreachable from every device. */
function isPrivateLanAddress(address: string): boolean {
	const [a, b] = address.split('.').map(Number);
	return a === 10 || a === 192 || (a === 172 && b >= 16 && b <= 31);
}

/** Bridges, containers and VPNs hold private addresses no phone on the LAN
 *  can reach: docker0 sits on 172.17.0.1. */
const VIRTUAL_INTERFACE =
	/^(docker|br-|virbr|veth|tun|utun|tap|tailscale|wg|zt|vEthernet)/;

/** Built-in wifi/ethernet ahead of USB and virtual interfaces, lowest number
 *  first: en0, en1 on macOS; eth0, enp3s0, wlan0, wlp2s0 on Linux. A tethered
 *  iPhone sharing its connection hands out a private address too, and it must
 *  not win over the real LAN. */
function interfaceRank(name: string): number {
	if (VIRTUAL_INTERFACE.test(name)) return 2000;
	const nic = /^(?:en|eth|wl)[a-z]*(\d+)/.exec(name);
	return nic === null ? 1000 : Number(nic[1]);
}

/** The host's IPv4 on the LAN, which phones reach the host's servers at, or
 *  `null` when it has none. `E2E_HOST_IP` overrides the detection. */
export function lanHostAddress(): HostAddress | null {
	const override = process.env.E2E_HOST_IP;
	if (override !== undefined && override !== '') {
		return { name: 'E2E_HOST_IP', address: override };
	}
	const usable = nonInternalIpv4s()
		.filter(({ address }) => isPrivateLanAddress(address))
		.sort((a, b) => interfaceRank(a.name) - interfaceRank(b.name));
	return usable[0] ?? null;
}
