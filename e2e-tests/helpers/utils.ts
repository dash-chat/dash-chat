/** `message` stamped with the host's wall-clock time, to line a spec line
 *  up with the device logs. */
export function stampedLog(message: string): void {
	console.log(`${new Date().toISOString().slice(11, 23)} ${message}`);
}

/** Integer from the environment, or `fallback` when the variable is unset. */
export function envInt(name: string, fallback: number): number {
	const raw = process.env[name];
	if (raw === undefined || raw === '') return fallback;
	const value = Number(raw);
	if (!Number.isFinite(value)) {
		throw new Error(`${name} must be a number, got "${raw}"`);
	}
	return value;
}
