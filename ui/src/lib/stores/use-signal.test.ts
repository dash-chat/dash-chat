import { reactive, relay, watcher } from 'signalium';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { getKeepAliveScope } from './keep-alive-scope.svelte';
import { useReactivePromises } from './use-signal';

vi.mock('./keep-alive-scope.svelte', () => ({
	getKeepAliveScope: vi.fn(() => undefined),
	trackRpVersion: (rp: { _version: { value: unknown } }) => rp._version.value,
}));

type RelayState = { value: number | undefined; setError(error: unknown): void };
type Versioned = { _version: { value: unknown } };

const nextFlush = () => new Promise(resolve => setTimeout(resolve, 20));

function relaySource(initial: number) {
	const stats: { state?: RelayState; stopped: number } = { stopped: 0 };
	const source = reactive(() =>
		relay<number>(s => {
			stats.state = s as RelayState;
			s.value = initial;
			return () => {
				stats.stopped++;
			};
		}),
	);
	return { source, stats };
}

/** A source another subscriber already holds open, like the chat list does. */
async function warmSource(initial: number) {
	const { source, stats } = relaySource(initial);
	const holder = watcher(() => {
		const rp = source();
		(rp as unknown as Versioned)._version.value;
		return rp.value;
	});
	const release = holder.addListener(() => {});
	holder.value;
	await nextFlush();
	return { source, stats, release };
}

function collect(store: {
	subscribe(run: (value: unknown) => void): () => void;
}) {
	const emissions: unknown[] = [];
	const unsubscribe = store.subscribe(value => {
		if (value instanceof Promise) value.catch(() => {});
		emissions.push(value);
	});
	return { emissions, unsubscribe };
}

describe('useReactivePromises', () => {
	beforeEach(() => {
		vi.mocked(getKeepAliveScope).mockReturnValue(undefined as never);
	});

	it('delivers warm sources during subscribe, exactly once', async () => {
		const a = await warmSource(1);
		const b = await warmSource(2);
		const { emissions, unsubscribe } = collect(
			useReactivePromises(() => [a.source(), b.source()]),
		);

		expect(emissions).toEqual([[1, 2]]);
		await nextFlush();
		expect(emissions).toEqual([[1, 2]]);

		unsubscribe();
		a.release();
		b.release();
	});

	it('surfaces a warm source that rejects right after subscribing', async () => {
		const a = await warmSource(1);
		const { emissions, unsubscribe } = collect(
			useReactivePromises(() => [a.source()]),
		);

		const failure = new Error('source failed');
		a.stats.state!.setError(failure);
		await nextFlush();

		const last = emissions.at(-1);
		expect(last).toBeInstanceOf(Promise);
		await expect(last).rejects.toBe(failure);

		unsubscribe();
		a.release();
	});

	it('keeps relay sources open for its keep-alive scope after the subscriber leaves', async () => {
		const holders: (() => void)[] = [];
		vi.mocked(getKeepAliveScope).mockReturnValue({
			keepAlive(fn: (...a: unknown[]) => unknown, args: unknown[]) {
				const holder = watcher(() => {
					(fn(...args) as Versioned)._version.value;
					return undefined;
				});
				holders.push(holder.addListener(() => {}));
			},
		} as never);

		const { source, stats } = relaySource(1);
		const { unsubscribe } = collect(useReactivePromises(() => [source()]));
		await nextFlush();
		unsubscribe();
		await nextFlush();
		expect(stats.stopped).toBe(0);

		holders.forEach(release => release());
		await nextFlush();
		expect(stats.stopped).toBe(1);
	});
});
