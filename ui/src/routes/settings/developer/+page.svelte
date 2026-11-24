<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
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
	import { getContext } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');

	async function disableDeveloperMode() {
		await settingsStore.setDeveloperModeEnabled(false);
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
