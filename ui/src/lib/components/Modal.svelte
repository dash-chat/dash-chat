<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { portalToModalHost } from '$lib/actions/portal-to-modal-host';
	import { suppressKeyboardRestore } from 'tauri-plugin-virtual-keyboard';

	interface ModalControls {
		opened: boolean;
		close: () => void;
	}

	interface Props {
		opened: boolean;
		/** Called once the exit animation has finished and the overlay has left
		 * the DOM. */
		onClosed?: () => void;
		children: Snippet<[ModalControls]>;
	}

	let { opened = $bindable(false), onClosed, children }: Props = $props();

	type Phase = 'unmounted' | 'entering' | 'open' | 'leaving';

	const EXIT_DURATION = 400;

	let phase = $state<Phase>('unmounted');

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

	// Keeps the overlay out of the DOM until it is needed, without losing the
	// animations it plays on the way in and out.
	$effect(() => {
		// Untracked: a tracked read would re-run this effect on every phase
		// change, restarting the transition it just scheduled.
		const current = untrack(() => phase);

		if (opened) {
			if (current === 'open') return;
			phase = 'entering';
			return afterNextPaint(() => (phase = 'open'));
		}

		if (current === 'unmounted') return;
		phase = 'leaving';
		return afterDelay(EXIT_DURATION, () => {
			phase = 'unmounted';
			onClosed?.();
		});
	});

	$effect(() => {
		if (!opened) return;
		return suppressKeyboardRestore();
	});

	function close() {
		opened = false;
	}
</script>

{#if phase !== 'unmounted'}
	<div class="contents" {@attach portalToModalHost}>
		{@render children({ opened: phase === 'open', close })}
	</div>
{/if}
