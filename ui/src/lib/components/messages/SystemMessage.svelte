<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import {
		mdiAccountMinusOutline,
		mdiAccountMultipleOutline,
		mdiAccountPlusOutline,
		mdiShieldAccountOutline,
	} from '@mdi/js';
	import type { GroupControlEvent } from 'dash-chat-stores';
	import { groupEventText } from '$lib/utils/group-event-text';
	import { wrapPathInSvg } from '$lib/utils/icon';

	let { event }: { event: GroupControlEvent } = $props();

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
		}
	});
</script>

<div
	class="flex items-center justify-center gap-1.5 py-1 text-sm quiet text-center"
	data-testid={`group-chat-system-message-${event.kind}`}
>
	<wa-icon
		class="shrink-0"
		src={wrapPathInSvg(iconPath)}
		style="font-size: 1rem;"
	></wa-icon>
	<span>{groupEventText(event)}</span>
</div>
