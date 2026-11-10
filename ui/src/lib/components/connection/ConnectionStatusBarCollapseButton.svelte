<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiChevronDoubleUp } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let { onclick }: { onclick: () => void } = $props();
</script>

<button
	type="button"
	data-testid="connection-status-bar-collapse"
	class="flex w-[35px] cursor-pointer justify-center"
	aria-label={m.connectionStatusHideDetailsAriaLabel()}
	{onclick}
>
	<wa-icon class="text-2xl" src={wrapPathInSvg(mdiChevronDoubleUp)}></wa-icon>
</button>
