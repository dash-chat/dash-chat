/** Deterministic 32-bit hash of a string, for stable color assignment. */
export function hashCode(s: string): number {
	let hash = 0;
	for (let i = 0; i < s.length; i++) {
		hash = (hash * 31 + s.charCodeAt(i)) >>> 0;
	}
	return hash;
}
