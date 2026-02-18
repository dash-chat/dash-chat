/**
 * Creates a memoized function that caches results by key.
 * Similar to signalium's reactive(), but with key-based memoization.
 *
 * @param factory - A function that creates a value for a given key
 * @returns A function that returns cached values or creates new ones
 */
export function memo<K extends string | number, V>(
	factory: (key: K) => V,
): (key: K) => V {
	const cache: Record<K, V> = {} as Record<K, V>;

	return (key: K): V => {
		if (!(key in cache)) {
			cache[key] = factory(key);
		}
		return cache[key];
	};
}
