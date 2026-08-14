<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { modalHost } from '$lib/stores/modal-host.svelte';
	import { suppressKeyboardRestore } from 'tauri-plugin-virtual-keyboard';

	interface ModalControls {
		opened: boolean;
		close: () => void;
	}

	interface Props {
		opened: boolean;
		children: Snippet<[ModalControls]>;
	}

	let { opened = $bindable(false), children }: Props = $props();

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
		return afterDelay(EXIT_DURATION, () => (phase = 'unmounted'));
	});

	$effect(() => {
		if (!opened) return;
		return suppressKeyboardRestore();
	});

	function close() {
		opened = false;
	}

	// Konsta overlays are `position: fixed`. The virtual keyboard's FLIP writes
	// inline transforms on the message list and the composer bar, which makes
	// any fixed descendant resolve against them instead of the viewport.
	function portal(node: HTMLElement) {
		const host = modalHost.element;
		if (!host) return;
		host.append(node);
		return () => node.remove();
	}
</script>

{#if phase !== 'unmounted'}
	<div class="contents" {@attach portal}>
		{@render children({ opened: phase === 'open', close })}
	</div>
{/if}
