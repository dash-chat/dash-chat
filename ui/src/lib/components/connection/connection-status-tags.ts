// Kept free of `$lib` imports so the e2e tsconfig can compile it through
// ui/tests/setup-utils.ts.
export const CONNECTION_STATUS_TAGS = [
	'cloud',
	'local',
	'p2p',
	'disconnected',
	'pending',
] as const;

export type ConnectionStatusTag = (typeof CONNECTION_STATUS_TAGS)[number];
