<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import {
		mdiAccountCheckOutline,
		mdiAccountMinusOutline,
		mdiAccountMultipleOutline,
		mdiAccountPlusOutline,
		mdiCancel,
		mdiShieldAccountOutline,
	} from '@mdi/js';
	import {
		type SystemEvent,
		systemEventText,
	} from '$lib/utils/system-event-text';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let { event }: { event: SystemEvent } = $props();

	const iconPath = $derived.by(() => {
		switch (event.kind) {
			case 'group_created':
				return mdiAccountMultipleOutline;
			case 'group_member_added':
				return mdiAccountPlusOutline;
			case 'group_member_removed':
				return mdiAccountMinusOutline;
			case 'group_member_promoted':
			case 'group_member_demoted':
				return mdiShieldAccountOutline;
			case 'contact_blocked':
				return mdiCancel;
			case 'contact_unblocked':
				return mdiAccountCheckOutline;
		}
	});
</script>

<div
	class="flex items-center justify-center gap-1.5 py-1 text-sm quiet text-center"
	data-testid={`system-message-${event.kind}`}
>
	<wa-icon
		class="shrink-0"
		src={wrapPathInSvg(iconPath)}
		style="font-size: 1rem;"
	></wa-icon>
	<span>{systemEventText(event)}</span>
</div>
