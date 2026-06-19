<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose } from '@mdi/js';

	let {
		onRemove,
		removeTestId,
		class: klass = '',
		children,
	}: {
		onRemove: () => void;
		removeTestId: string;
		class?: string;
		children: Snippet;
	} = $props();
</script>

<div
	class="staged-thumb relative h-[120px] w-[120px] shrink-0 overflow-hidden {klass}"
>
	{@render children()}
	<div
		class="thumb-gradient pointer-events-none absolute inset-x-0 top-0 h-8"
	></div>
	<button
		type="button"
		class="thumb-remove absolute end-1 top-1 flex h-4 w-4 items-center justify-center p-0"
		data-testid={removeTestId}
		aria-label={m.removeAttachment()}
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

	.thumb-gradient {
		background: linear-gradient(rgba(0, 0, 0, 0.4), transparent);
		opacity: 0;
		transition: opacity 0.15s ease;
	}
	.staged-thumb:hover .thumb-gradient {
		opacity: 1;
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
