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
		class:dot-gap={status === 'pending'}
		class:exclamation-gap={status === 'disconnected'}
	>
		{@html glyphs[status]}
	</span>
	{#if status === 'pending'}
		<span
			class="absolute -end-[1px] -bottom-[1px] size-2 rounded-full dark:bg-black dark:ring-2 dark:ring-black"
		>
			<span class="pending-dot block size-full rounded-full bg-[#969696]"
			></span>
		</span>
	{:else if status === 'disconnected'}
		<span
			class="absolute -end-[3px] -bottom-[3px] size-3 rounded-full bg-[#FF383C] text-white *:size-full dark:text-[#010101] dark:ring-2 dark:ring-black"
		>
			{@html exclamationSvg}
		</span>
	{/if}
</span>

<style>
	/* Figma's light ring matches its white bubble, so it only shows where it cuts
	   the icon. On translucent glass that takes a real gap, not a coloured ring. */
	.dot-gap,
	.exclamation-gap {
		--gap-x: 21px;
		mask-image: radial-gradient(
			circle var(--gap-radius) at var(--gap-x) 21px,
			transparent 99%,
			black 100%
		);
	}
	.dot-gap {
		--gap-radius: 6px;
	}
	.exclamation-gap {
		--gap-radius: 8px;
	}
	:global([dir='rtl']) .dot-gap,
	:global([dir='rtl']) .exclamation-gap {
		--gap-x: 3px;
	}
	.pending-dot {
		animation: pending-dot-pulse 1.2s ease-in-out infinite;
	}
	@keyframes pending-dot-pulse {
		50% {
			opacity: 0.15;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.pending-dot {
			animation: none;
			opacity: 0.5;
		}
	}
</style>
