<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';
	import { mdiReload } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		bytes: number;
		total: number;
		stalled: boolean;
		/** Outer diameter in px. */
		size?: number;
	}

	let { bytes, total, stalled, size = 40 }: Props = $props();

	const STROKE = 3;
	const MIN_DETERMINATE_FRACTION = 0.05;
	const radius = $derived(size / 2 - STROKE);
	const circumference = $derived(2 * Math.PI * radius);
	const fraction = $derived(
		total > 0 ? Math.min(1, Math.max(0, bytes / total)) : 0,
	);
	// Spinning until the arc has something to show: an unknown total, or the
	// first bytes, would otherwise snap the spinner to a near-empty ring.
	const indeterminate = $derived(
		!stalled && (total <= 0 || fraction < MIN_DETERMINATE_FRACTION),
	);
	const dashOffset = $derived(
		indeterminate ? circumference * 0.75 : circumference * (1 - fraction),
	);
	// A stalled ring is a retry affordance (the parent handles the tap), not a
	// progressbar, so it carries a label instead of a value range.
	const aria: HTMLAttributes<HTMLDivElement> = $derived(
		stalled
			? {
					role: 'img',
					'aria-label': m.blobDownloadStalledRetry(),
					title: m.blobDownloadStalledRetry(),
				}
			: {
					role: 'progressbar',
					'aria-busy': true,
					'aria-valuemin': 0,
					'aria-valuemax': total > 0 ? total : undefined,
					'aria-valuenow': indeterminate ? undefined : bytes,
				},
	);
</script>

<div
	class="relative inline-flex items-center justify-center"
	style="width: {size}px; height: {size}px;"
	data-testid="blob-progress-ring"
	data-stalled={stalled}
	{...aria}
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
	{/if}
</div>
