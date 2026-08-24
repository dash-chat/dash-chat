<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { developerMode } from '$lib/stores/developer-mode.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockTitle,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';

	const theme = $derived(useTheme());

	function disableDeveloperMode() {
		developerMode.lock();
		showToast(m.developerModeDisabled());
		goto('/settings');
	}
</script>

<Page>
	<Navbar title={m.developer()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="developer-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.developer()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem
					title={m.disableDeveloperMode()}
					link
					chevron={false}
					onClick={disableDeveloperMode}
					data-testid="developer-disable"
					colors={{
						primaryTextIos: 'text-red-500',
						primaryTextMaterial: 'text-red-500',
					}}
				/>
			</List>
		</div>
	</div>
</Page>
