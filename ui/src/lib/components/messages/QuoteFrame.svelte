<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		mine = false,
		composer = false,
		children,
	}: {
		/** Whether the frame sits inside my own (brand-colored) bubble. */
		mine?: boolean;
		/** Whether the frame sits in the composer's reply banner rather than a bubble. */
		composer?: boolean;
		children: Snippet;
	} = $props();
</script>

<span
	class="quote-frame"
	class:quote-frame-mine={mine}
	class:quote-frame-others={!mine}
	class:quote-frame-composer={composer}
>
	<span class="quote-frame-bar"></span>
	{@render children()}
</span>

<style>
	.quote-frame {
		--ios-quote-surface: #dfdfe1;
		--ios-quote-bar: #c1c1c3;
		display: flex;
		align-items: stretch;
		width: 100%;
		min-width: 0;
		/* A <button>'s intrinsic block size does not follow its flex content, so
		   the author and text lines get shrunk and clipped without this. */
		height: fit-content;
		border-radius: 10px 10px 4px 4px;
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
		background-color: rgba(0, 0, 0, 0.4);
		color: white;
	}

	.quote-frame-others {
		background-color: rgba(255, 255, 255, 0.55);
	}
	:global(.dark) .quote-frame-others {
		background-color: rgba(255, 255, 255, 0.16);
	}

	:global(.k-ios) .quote-frame {
		border-radius: 0.5rem;
	}

	/* The white tint and bar vanish on iOS's white incoming bubbles, which
	   app.css paints under this same guard. */
	:global(:root:not(.dark):not(.wide-layout) .k-ios)
		.quote-frame-others:not(.quote-frame-composer) {
		background-color: var(--ios-quote-surface);
	}
	:global(:root:not(.dark):not(.wide-layout) .k-ios)
		.quote-frame-others:not(.quote-frame-composer)
		.quote-frame-bar {
		background-color: var(--ios-quote-bar);
	}

	.quote-frame-composer {
		border-radius: 18px 18px 4px 4px;
	}

	/* The iOS theme copies Signal's composer reply-banner greys. */
	:global(.k-ios) .quote-frame-composer {
		background-color: var(--ios-quote-surface);
		border-radius: 16px;
	}
	:global(.k-ios) .quote-frame-composer .quote-frame-bar {
		background-color: var(--ios-quote-bar);
	}
	:global(.dark .k-ios) .quote-frame-composer {
		background-color: #454547;
	}
	:global(.dark .k-ios) .quote-frame-composer .quote-frame-bar {
		background-color: #5f5e63;
	}
</style>
