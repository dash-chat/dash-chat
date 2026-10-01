import type { LinkConditions } from '../setup/toxiproxy';

/** A lost SYN or ClientHello: Linux ≥ 6.5 and iOS retry after 1 s. A lost
 *  uncached DNS query (5 s on Android) is left out: it is rare, and toxiproxy
 *  would charge it again on every request over the connection it hit, far
 *  past what a lost lookup costs. */
function lostSetupPacket(share: number) {
	return { share, extraMs: 1_000 };
}

/** Networks measured in the global majority, as toxiproxy conditions.
 *
 *  Setting up a connection costs a phone two round trips, TCP and TLS, but
 *  toxiproxy only slows the TLS one. So the latency each way is the whole
 *  measured round trip: setup costs what it does in the field, and requests on
 *  an open connection pay double. */
export const NETWORK_PROFILES: Record<string, LinkConditions> = {
	// 500 ± 300 ms round trips: Africa or South Asia to the EU under moderate
	// load (AmiGos, arXiv 2209.04129: 108–149 ms idle to nearby services);
	// Lighthouse's slow 4G bandwidth.
	'congested mobile': {
		latencyMs: 500,
		jitterMs: 300,
		rateKBps: 200,
		stall: lostSetupPacket(0.05),
	},
	// ~1.2 s round trips: Ookla open data Q2 2026, Mauritius mobile loaded
	// latency (1008 ms mean, in-country), plus ~200 ms to the EU (RIPE Atlas).
	'loaded mobile': {
		latencyMs: 1_200,
		jitterMs: 500,
		rateKBps: 200,
		stall: lostSetupPacket(0.05),
	},
	// Round trips of 50–209 ms at public hotspots (CTCP, arXiv 1212.2291) on
	// a ~230 ms path from Mauritius to the EU (RIPE Atlas).
	'venue wifi': {
		latencyMs: 250,
		jitterMs: 160,
		rateKBps: 250,
		stall: lostSetupPacket(0.1),
	},
	// Facebook ATC's Edge-Lossy: 840 ms round trips, 240 kbit/s, 1% loss.
	EDGE: {
		latencyMs: 840,
		jitterMs: 200,
		rateKBps: 30,
		stall: lostSetupPacket(0.05),
	},
	// Calibrated to the Sentry report from a Samsung A16 in Mauritius, where
	// most connection setups outlasted 5 s and a few took 3–4 s: ~2.5 s round
	// trips, the tail of Ookla's loaded latency there (tile p90 2.1 s
	// in-country) on the path to the EU.
	'Mauritius field report': {
		latencyMs: 2_600,
		jitterMs: 700,
		rateKBps: 200,
		stall: lostSetupPacket(0.05),
	},
};
