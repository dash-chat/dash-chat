<!--
	A sticky day separator pill that docks just below the page's `<Navbar>`
	rather than at the scroll-viewport top — which would otherwise leave it
	eclipsed by a sticky/transparent navbar.

	The `use:navbarSticky` action handles tracking the navbar's height; all
	day tags inside the same `.k-page` share one set of observers.

	Renders a localised date label whose granularity depends on how recent the
	day is — "today" / "yesterday" / weekday + day for this week / longer for
	older. The raw ISO date is exposed on `data-day` so callers can locate a
	specific separator (e.g. for a date-jump search action).

	Usage:
	  <DayTag day={messageSetInDay.day} />
-->
<script lang="ts">
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m } from '$lib/paraglide/messages.js';
	import {
		beforeYesterday,
		inYesterday,
		moreThanAYearAgo,
	} from '$lib/utils/time';
	import { navbarSticky } from '$lib/actions/navbar-sticky';

	interface Props {
		day: Date;
		class?: string;
	}

	let { day, class: className = '' }: Props = $props();
</script>

<div use:navbarSticky class="day-tag {className}" data-day={day.toISOString()}>
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
		align-self: center;
		z-index: 10;
		background-color: var(--k-color-md-light-surface);
		padding: 4px 12px;
		border-radius: 12px;
		font-size: 0.75rem;
	}

	:global(.dark) .day-tag {
		background-color: var(--k-color-md-dark-surface);
	}
</style>
