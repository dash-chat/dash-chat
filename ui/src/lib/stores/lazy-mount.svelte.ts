import { untrack } from 'svelte';

type Phase = 'unmounted' | 'entering' | 'open' | 'leaving';

function afterNextPaint(fn: () => void): () => void {
	let second = 0;
	const first = requestAnimationFrame(
		() => (second = requestAnimationFrame(fn)),
	);
	return () => {
		cancelAnimationFrame(first);
		cancelAnimationFrame(second);
	};
}

function afterDelay(ms: number, fn: () => void): () => void {
	const timeout = setTimeout(fn, ms);
	return () => clearTimeout(timeout);
}

/**
 * Keep an overlay out of the DOM until it is needed, without losing the
 * animations it plays on the way in and out.
 */
export function lazyMount(opened: () => boolean, exitDuration = 400) {
	let phase = $state<Phase>('unmounted');

	$effect(() => {
		// Untracked: a tracked read would re-run this effect on every phase
		// change, restarting the transition it just scheduled.
		const current = untrack(() => phase);

		if (opened()) {
			if (current === 'open') return;
			phase = 'entering';
			return afterNextPaint(() => (phase = 'open'));
		}

		if (current === 'unmounted') return;
		phase = 'leaving';
		return afterDelay(exitDuration, () => (phase = 'unmounted'));
	});

	return {
		get mounted() {
			return phase !== 'unmounted';
		},
		get opened() {
			return phase === 'open';
		},
	};
}
