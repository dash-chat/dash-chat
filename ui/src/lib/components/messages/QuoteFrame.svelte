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
	}

	/* Inside my (brand-colored) bubble: translucent white over the brand color. */
	.quote-frame-mine {
		background-color: rgba(255, 255, 255, 0.18);
		color: white;
	}
	.quote-frame-mine .quote-frame-bar {
		background-color: rgba(255, 255, 255, 0.85);
	}

	/* On a surface: subtle tint + brand accent bar. */
	.quote-frame-others {
		background-color: rgba(0, 0, 0, 0.06);
	}
	:global(.dark) .quote-frame-others {
		background-color: rgba(255, 255, 255, 0.08);
	}
	.quote-frame-others .quote-frame-bar {
		background-color: var(--color-brand-primary);
	}
</style>
