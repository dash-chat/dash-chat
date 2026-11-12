<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiQrcode } from '@mdi/js';
	import { Link, useTheme } from 'konsta/svelte';
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages';
	import { wrapPathInSvg } from '$lib/utils/icon';

	const theme = $derived(useTheme());
</script>

{#snippet link()}
	<Link
		iconOnly
		aria-label={m.addContact()}
		data-testid="home-add-contact-btn"
		onClick={() =>
			// The sidebar's back button returns to where the user came from only
			// for an entry carrying this state; otherwise it goes home.
			goto('/new-message/add-contact', {
				state: { sidebarPanel: 'new-message' },
			})}
	>
		<wa-icon src={wrapPathInSvg(mdiQrcode)}> </wa-icon>
	</Link>
{/snippet}

{#if theme === 'ios'}
	<div
		class="grid size-11 place-items-center rounded-full bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass"
	>
		{@render link()}
	</div>
{:else}
	{@render link()}
{/if}
