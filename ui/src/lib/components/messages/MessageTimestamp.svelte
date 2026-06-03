<script lang="ts">
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m } from '$lib/paraglide/messages.js';
	import { lessThanAMinuteAgo, moreThanAnHourAgo } from '$lib/utils/time';

	let {
		timestamp,
		class: className = '',
	}: {
		timestamp: number;
		class?: string;
	} = $props();
</script>

<div class={`text-xs ${className}`}>
	{#if lessThanAMinuteAgo(timestamp)}
		<span>{m.now()}</span>
	{:else if moreThanAnHourAgo(timestamp)}
		<wa-format-date
			hour="numeric"
			minute="numeric"
			hour-format="24"
			date={new Date(timestamp)}
		></wa-format-date>
	{:else}
		<wa-relative-time sync format="narrow" date={new Date(timestamp)}
		></wa-relative-time>
	{/if}
</div>
