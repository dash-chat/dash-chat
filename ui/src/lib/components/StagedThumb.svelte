<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose } from '@mdi/js';

	let {
		removeLabel,
		onRemove,
		removeTestId,
		class: klass = '',
		children,
	}: {
		removeLabel: string;
		onRemove: () => void;
		removeTestId: string;
		class?: string;
		children: Snippet;
	} = $props();
</script>

<div class="staged-thumb relative shrink-0 overflow-hidden {klass}">
	{@render children()}
	<div
		class="thumb-gradient pointer-events-none absolute inset-x-0 top-0 h-8"
	></div>
	<button
		type="button"
		class="thumb-remove absolute end-1 top-1 flex h-4 w-4 items-center justify-center p-0"
		data-testid={removeTestId}
		aria-label={removeLabel}
		onclick={onRemove}
	>
		<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
	</button>
</div>

<style>
	.staged-thumb {
		border-radius: 4px;
		background: rgba(128, 128, 128, 0.1);
	}

	/* WebKitGTK paints one frame of the pre-transition opacity when an
	   accelerated opacity transition ends, so the fade runs on a registered
	   custom property, which the compositor cannot take over. */
	@property --thumb-gradient-alpha {
		syntax: '<number>';
		inherits: false;
		initial-value: 0;
	}
	.thumb-gradient {
		background: linear-gradient(
			rgba(0, 0, 0, calc(0.4 * var(--thumb-gradient-alpha))),
			transparent
		);
		transition: --thumb-gradient-alpha 0.15s ease;
	}
	.staged-thumb:hover .thumb-gradient {
		--thumb-gradient-alpha: 1;
	}

	.thumb-remove {
		border: none;
		background: transparent;
		cursor: pointer;
		color: white;
	}
	.thumb-remove :global(wa-icon) {
		width: 16px;
		height: 16px;
		filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.6));
	}
</style>
