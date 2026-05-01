<!--
	A sticky day separator pill that docks just below the page's `<Navbar>`
	rather than at the scroll-viewport top — which would otherwise leave it
	eclipsed by a sticky/transparent navbar.

	Finds the `.k-navbar` inside its containing `.k-page`, observes its size,
	and writes its own `top` accordingly. Re-resolves when the navbar element
	is swapped (e.g. when search mode toggles in/out).

	Renders a localised date label whose granularity depends on how recent the
	day is — "today" / "yesterday" / weekday + day for this week / longer for
	older. The raw ISO date is exposed on `data-day` so callers can locate a
	specific separator (e.g. for a date-jump search action).

	Usage:
	  <StickyDayTag day={messageSetInDay.day} />
-->
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
	let el: HTMLDivElement | null = $state(null);

	$effect(() => {
		const node = el;
		if (!node) return;
		const page = node.closest('.k-page');
		if (!page) return;

		let observed: HTMLElement | null = null;
		const ro = new ResizeObserver(() => {
			if (observed && node) {
				node.style.top = `${observed.offsetHeight + 8}px`;
			}
		});
		const sync = () => {
			const navbar = page.querySelector('.k-navbar') as HTMLElement | null;
			if (navbar === observed) return;
			if (observed) ro.unobserve(observed);
			observed = navbar;
			if (navbar) {
				ro.observe(navbar);
				node.style.top = `${navbar.offsetHeight + 8}px`;
			}
		};
		sync();
		const mo = new MutationObserver(sync);
		mo.observe(page, { childList: true, subtree: true });
		return () => {
			ro.disconnect();
			mo.disconnect();
		};
	});
</script>

<div
	bind:this={el}
	class="sticky-day-tag {className}"
	data-day={day.toISOString()}
>
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
	.sticky-day-tag {
		position: sticky;
		align-self: center;
		z-index: 10;
		background-color: var(--k-color-md-light-surface);
		padding: 4px 12px;
		border-radius: 12px;
		font-size: 0.75rem;
	}

	:global(.dark) .sticky-day-tag {
		background-color: var(--k-color-md-dark-surface);
	}
</style>
