<script lang="ts">
	import exclamationSvg from '$lib/assets/connection-status/exclamation.svg?raw';
	import hubSvg from '$lib/assets/connection-status/hub.svg?raw';
	import internetSvg from '$lib/assets/connection-status/internet.svg?raw';
	import p2pSvg from '$lib/assets/connection-status/p2p.svg?raw';
	import type { ConnectionStatusTag } from './connection-status-tags';

	let { status }: { status: ConnectionStatusTag } = $props();

	const glyphs: Record<ConnectionStatusTag, string> = {
		cloud: internetSvg,
		pending: internetSvg,
		local: hubSvg,
		p2p: p2pSvg,
		disconnected: internetSvg,
	};
</script>

<span class="relative block size-6">
	<span
		class="block size-full *:size-full"
		class:badge-gap={status === 'pending' || status === 'disconnected'}
	>
		{@html glyphs[status]}
	</span>
	{#if status === 'pending'}
		<svg
			class="spinner absolute end-0 -bottom-[2px] size-3 overflow-visible text-[#767676] dark:text-[#525252]"
			viewBox="0 0 12 12"
			fill="none"
		>
			<circle
				cx="6"
				cy="6"
				r="5"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-dasharray="25.13 6.29"
			/>
		</svg>
	{:else if status === 'disconnected'}
		<span
			class="absolute end-0 -bottom-[2px] size-3 rounded-full bg-[#FF383C] text-white *:size-full dark:text-[#010101]"
		>
			{@html exclamationSvg}
		</span>
	{/if}
</span>

<style>
	/* Figma's light ring matches its white bubble, so it only shows where it cuts
	   the icon. On translucent glass that takes a real gap, not a coloured ring. */
	.badge-gap {
		--gap-x: 18px;
		mask-image: radial-gradient(
			circle 8px at var(--gap-x) 20px,
			transparent 99%,
			black 100%
		);
	}
	:global([dir='rtl']) .badge-gap {
		--gap-x: 6px;
	}
	/* Moving the dash redraws the arc every frame; rotating the element would
	   turn a rasterised picture of it. */
	.spinner circle {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		to {
			stroke-dashoffset: -31.42;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.spinner circle {
			animation: spinner-pulse 2s ease-in-out infinite;
		}
	}
	@keyframes spinner-pulse {
		50% {
			opacity: 0.3;
		}
	}
</style>
