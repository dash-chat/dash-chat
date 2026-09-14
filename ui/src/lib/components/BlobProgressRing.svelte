<script lang="ts">
	import type { Snippet } from 'svelte';
	import { mdiReload } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		bytes: number;
		total: number;
		stalled: boolean;
		/** Outer diameter in px. */
		size?: number;
		class?: string;
		children?: Snippet;
	}

	let {
		bytes,
		total,
		stalled,
		size = 40,
		class: className = '',
		children,
	}: Props = $props();

	const STROKE = 3;
	const radius = $derived(size / 2 - STROKE);
	const circumference = $derived(2 * Math.PI * radius);
	const fraction = $derived(
		total > 0 ? Math.min(1, Math.max(0, bytes / total)) : 0,
	);
	const indeterminate = $derived(bytes === 0 && !stalled);
	const dashOffset = $derived(
		indeterminate ? circumference * 0.75 : circumference * (1 - fraction),
	);
</script>

<div
	class="relative inline-flex items-center justify-center {className}"
	style="width: {size}px; height: {size}px;"
	data-testid="blob-progress-ring"
	data-stalled={stalled}
	title={stalled ? m.blobDownloadStalledRetry() : undefined}
	aria-busy={!stalled && fraction < 1}
>
	<svg
		class="absolute inset-0 -rotate-90 {indeterminate ? 'animate-spin' : ''}"
		width={size}
		height={size}
		viewBox="0 0 {size} {size}"
		aria-hidden="true"
	>
		<circle
			cx={size / 2}
			cy={size / 2}
			r={radius}
			fill="none"
			stroke="currentColor"
			stroke-opacity="0.25"
			stroke-width={STROKE}
		/>
		<circle
			cx={size / 2}
			cy={size / 2}
			r={radius}
			fill="none"
			stroke="currentColor"
			stroke-width={STROKE}
			stroke-linecap="round"
			stroke-dasharray={circumference}
			stroke-dashoffset={dashOffset}
		/>
	</svg>
	{#if stalled}
		<svg
			viewBox="0 0 24 24"
			width={size * 0.5}
			height={size * 0.5}
			aria-hidden="true"
		>
			<path fill="currentColor" d={mdiReload} />
		</svg>
	{:else if children}
		{@render children()}
	{/if}
</div>
