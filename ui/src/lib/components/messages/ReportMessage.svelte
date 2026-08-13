<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import { mdiAlertOctagonOutline } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
</script>

<div
	class="flex items-center justify-center gap-1.5 py-1 text-sm quiet text-center"
	data-testid="direct-chat-report-message"
>
	<wa-icon
		class="shrink-0"
		src={wrapPathInSvg(mdiAlertOctagonOutline)}
		style="font-size: 1rem;"
	></wa-icon>
	<span>{m.youReportedThisContact()}</span>
</div>
