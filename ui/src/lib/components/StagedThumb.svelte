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

<div
	class="group relative shrink-0 overflow-hidden rounded-sm bg-[rgba(128,128,128,0.1)] {klass}"
>
	{@render children()}
	<div
		class="pointer-events-none absolute inset-x-0 top-0 h-8 bg-linear-to-b from-black/40 to-transparent opacity-0 transition-opacity duration-150 group-hover:opacity-100"
	></div>
	<button
		type="button"
		class="absolute end-1 top-1 flex size-4 cursor-pointer items-center justify-center rounded-full border-none bg-transparent p-0 text-white pointer-coarse:size-8 pointer-coarse:bg-black/45"
		data-testid={removeTestId}
		aria-label={removeLabel}
		onclick={onRemove}
	>
		<wa-icon
			class="!text-[16px] drop-shadow-[0_0_2px_rgba(0,0,0,0.6)] pointer-coarse:!text-xl pointer-coarse:drop-shadow-none"
			src={wrapPathInSvg(mdiClose)}
		></wa-icon>
	</button>
</div>
