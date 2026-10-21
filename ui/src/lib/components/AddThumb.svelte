<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPlus } from '@mdi/js';

	let {
		onClick,
		label,
		testId,
		disabled = false,
		class: klass = '',
	}: {
		onClick: () => void;
		label: string;
		testId: string;
		disabled?: boolean;
		class?: string;
	} = $props();
</script>

<!-- WebKit ignores aspect-ratio on a button, so the wrapper carries the size. -->
<div class="shrink-0 {klass}">
	<button
		type="button"
		class="flex h-full w-full cursor-pointer items-center justify-center rounded-sm border-2 border-dashed border-[var(--k-hairline-color,rgba(128,128,128,0.4))] bg-transparent text-[var(--k-text-color)] opacity-60 transition-opacity duration-150 enabled:hover:opacity-90 disabled:cursor-default disabled:opacity-30"
		data-testid={testId}
		aria-label={label}
		{disabled}
		onclick={onClick}
	>
		<wa-icon class="size-7" src={wrapPathInSvg(mdiPlus)}></wa-icon>
	</button>
</div>
