<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		mine = false,
		children,
	}: {
		/** Whether the frame sits inside my own (brand-colored) bubble. */
		mine?: boolean;
		children: Snippet;
	} = $props();
</script>

<span class="quote-frame {mine ? 'quote-frame-mine' : 'quote-frame-others'}">
	<span class="quote-frame-bar"></span>
	{@render children()}
</span>

<style>
	.quote-frame {
		display: flex;
		align-items: stretch;
		width: 100%;
		min-width: 0;
		/* A <button>'s intrinsic block size does not follow its flex content, so
		   the author and text lines get shrunk and clipped without this. */
		height: fit-content;
		border-radius: 0.5rem;
		overflow: hidden;
		text-align: start;
	}

	.quote-frame-bar {
		flex-shrink: 0;
		width: 4px;
		background-color: white;
	}

	.quote-frame-mine {
		background-color: rgba(255, 255, 255, 0.7);
		color: rgba(0, 0, 0, 0.87);
	}
	:global(.dark) .quote-frame-mine {
		background-color: rgba(255, 255, 255, 0.45);
	}

	.quote-frame-others {
		background-color: rgba(255, 255, 255, 0.55);
	}
	:global(.dark) .quote-frame-others {
		background-color: rgba(255, 255, 255, 0.16);
	}
</style>
