<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCalendarSearch, mdiChevronDown, mdiChevronUp } from '@mdi/js';

	interface Props {
		current: number;
		total: number;
		hasQuery: boolean;
		onPrevious: () => void;
		onNext: () => void;
		onJumpToDate: (dateStr: string) => void;
	}

	let { current, total, hasQuery, onPrevious, onNext, onJumpToDate }: Props =
		$props();

	let dateInput = $state<HTMLInputElement>();
</script>

<div class="row items-center gap-2 px-4 py-3">
	<button onclick={() => dateInput?.click()} aria-label={m.jumpToDate()}>
		<wa-icon class="quiet" src={wrapPathInSvg(mdiCalendarSearch)}></wa-icon>
	</button>
	<input
		type="date"
		class="absolute opacity-0 h-0 w-0"
		bind:this={dateInput}
		onchange={e => onJumpToDate(e.currentTarget.value)}
	/>
	<span
		class="flex-1 text-center text-sm quiet"
		data-testid="search-results-count"
	>
		{#if !hasQuery}
			<!-- empty -->
		{:else if total === 0}
			{m.noResults()}
		{:else}
			{m.searchResultsCount({
				current: String(current),
				total: String(total),
			})}
		{/if}
	</span>
	<button
		disabled={total === 0}
		onclick={onPrevious}
		class="flex h-8 w-8 items-center justify-center disabled:opacity-30"
		aria-label={m.previousResult()}
	>
		<wa-icon src={wrapPathInSvg(mdiChevronUp)}></wa-icon>
	</button>
	<button
		disabled={total === 0}
		onclick={onNext}
		class="flex h-8 w-8 items-center justify-center disabled:opacity-30"
		aria-label={m.nextResult()}
	>
		<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
	</button>
</div>
