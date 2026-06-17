<script lang="ts">
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m } from '$lib/paraglide/messages.js';
	import {
		beforeYesterday,
		inYesterday,
		moreThanAYearAgo,
	} from '$lib/utils/time';

	interface Props {
		day: Date;
		class?: string;
	}

	let { day, class: className = '' }: Props = $props();
</script>

<div class="day-tag {className}" data-day={day.toISOString()}>
	{#if moreThanAYearAgo(day.valueOf())}
		<wa-format-date month="numeric" year="numeric" day="numeric" date={day}
		></wa-format-date>
	{:else if beforeYesterday(day.valueOf())}
		<wa-format-date month="short" day="numeric" weekday="narrow" date={day}
		></wa-format-date>
	{:else if inYesterday(day.valueOf())}
		{m.yesterday()}
	{:else}
		{m.today()}
	{/if}
</div>

<style>
	.day-tag {
		background-color: var(--color-page-surface);
		padding: 4px 12px;
		border-radius: 12px;
		font-size: 0.85rem;
	}
</style>
