<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiMicrophone } from '@mdi/js';
	import { formatDuration } from '$lib/utils/time';

	interface Props {
		elapsedMs: number;
		micSize?: number;
		timerTestid?: string;
	}

	let { elapsedMs, micSize = 22, timerTestid }: Props = $props();
</script>

<wa-icon
	class="rec-mic"
	style="width: {micSize}px; height: {micSize}px"
	src={wrapPathInSvg(mdiMicrophone)}
></wa-icon>
<span class="text-sm tabular-nums" data-testid={timerTestid}
	>{formatDuration(elapsedMs)}</span
>

<style>
	.rec-mic {
		color: #ef4444;
		animation: pulse 1.2s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.3;
		}
	}
</style>
