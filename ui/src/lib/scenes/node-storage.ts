type Stored = Record<string, string>;

function memoryStorage(): Stored {
	return Object.create({
		getItem(this: Stored, key: string) {
			return Object.hasOwn(this, key) ? this[key] : null;
		},
		setItem(this: Stored, key: string, value: string) {
			this[key] = value;
		},
		removeItem(this: Stored, key: string) {
			delete this[key];
		},
		clear(this: Stored) {
			for (const key of Object.keys(this)) delete this[key];
		},
	});
}

// The mock backend keeps everything in the page's storage, and scenes write
// to it in Node, which has none: these stand in for it, and what they hold is
// what the page is then started with.
const stored = { local: memoryStorage(), session: memoryStorage() };

Object.defineProperty(globalThis, 'localStorage', {
	value: stored.local,
	configurable: true,
});
Object.defineProperty(globalThis, 'sessionStorage', {
	value: stored.session,
	configurable: true,
});

export function clearStorage(): void {
	localStorage.clear();
	sessionStorage.clear();
}

export function storedEntries(): {
	local: [string, string][];
	session: [string, string][];
} {
	return {
		local: Object.entries(stored.local),
		session: Object.entries(stored.session),
	};
}
