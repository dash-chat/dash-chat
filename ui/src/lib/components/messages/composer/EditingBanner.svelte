<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiPencilOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
</script>

<div
	class="flex items-center gap-1.5 ps-3 pt-2 text-sm font-semibold"
	data-testid="composer-editing-banner"
>
	<wa-icon src={wrapPathInSvg(mdiPencilOutline)} style="font-size: 0.9rem"
	></wa-icon>
	{m.editingMessage()}
</div>
