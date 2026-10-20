// Kept free of `$lib` imports so the e2e tsconfig can compile it through
// ui/tests/setup-utils.ts.
export const CONNECTION_CHIP_STATUSES = [
	'cloud',
	'local',
	'p2p',
	'disconnected',
	'pending',
] as const;

export type ConnectionChipStatus = (typeof CONNECTION_CHIP_STATUSES)[number];
