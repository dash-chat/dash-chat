<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiMicrophone } from '@mdi/js';
	import { formatDuration } from '$lib/utils/time';

	interface Props {
		elapsedMs: number;
		micSize?: number;
	}

	let { elapsedMs, micSize = 22 }: Props = $props();
</script>

<wa-icon
	class="rec-mic text-red-500"
	style="width: {micSize}px; height: {micSize}px"
	src={wrapPathInSvg(mdiMicrophone)}
></wa-icon>
<span class="text-sm tabular-nums" data-testid="voice-recording-timer"
	>{formatDuration(elapsedMs)}</span
>

<style>
	.rec-mic {
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
