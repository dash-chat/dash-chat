<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';

	let {
		show,
		testId,
		children,
	}: { show: boolean; testId: string; children: Snippet } = $props();
</script>

<span class="p2p-badged relative inline-flex">
	<span class="badged inline-flex">
		{@render children()}
	</span>
	{#if show}
		<span
			class="badge absolute end-0 bottom-0 size-[10.5px] rounded-full bg-[#22C55E]"
			role="img"
			aria-label={m.chatRowP2pBadgeLabel()}
			data-testid={testId}
		></span>
	{/if}
</span>

<style>
	/* The ring is cut out of the avatar rather than painted, because the row
	   behind it changes colour: page, desktop sidebar, selected row. */
	.p2p-badged:has(.badge) .badged {
		mask-image: radial-gradient(
			circle at calc(100% - 5.25px) calc(100% - 5.25px),
			transparent 7.625px,
			black 8.125px
		);
	}
	:global([dir='rtl']) .p2p-badged:has(.badge) .badged {
		mask-image: radial-gradient(
			circle at 5.25px calc(100% - 5.25px),
			transparent 7.625px,
			black 8.125px
		);
	}
</style>
