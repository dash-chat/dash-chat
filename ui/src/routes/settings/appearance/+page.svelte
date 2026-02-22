<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import {
		BlockTitle,
		Dialog,
		DialogButton,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Radio,
		useTheme,
	} from 'konsta/svelte';

	import { showToast } from '$lib/utils/toasts';
	import { getContext } from 'svelte';
	import type { PreferencesData, PreferencesStore } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';

	const store: PreferencesStore = getContext('preferences-store');
	const prefs = useReactivePromise(store.preferences);

	const theme = $derived(useTheme());
	let showThemeDialog = $state(false);

	function setThemeAndClose(theme: PreferencesData['appearanceTheme']) {
		store.setTheme(theme)
		showThemeDialog = false
	}
</script>

<Page>
	{#await $prefs then prefs}
		{#if prefs}
			<Navbar title={m.appearance()} titleClass="opacity1" transparent={true}>
				{#snippet left()}
					{#if !isWideScreen.value}
						<NavbarBackLink
							onClick={() => goto('/settings')}
							data-testid="account-back"
						/>
					{/if}
				{/snippet}
			</Navbar>

			<div class="column" style="flex: 1">
				<div class="column center-in-desktop">
					<List strongIos inset={isWideScreen.value || theme === 'ios'}>
						<ListItem
							title="Theme"
							after={prefs.appearanceTheme}
							link
							chevron={false}
							onClick={() => (showThemeDialog = true)}
							data-testid="appearance-theme"
						/>
					</List>
				</div>
			</div>
			<Dialog
				opened={showThemeDialog}
				onBackdropClick={() => (showThemeDialog = false)}
			>
				<List nested class="-mx-4">
					<ListItem label title="System">
						{#snippet after()}
							<Radio
								component="div"
								value="system"
								checked={prefs.appearanceTheme === 'system'}
								onChange={() => setThemeAndClose('system')}
							/>
						{/snippet}
					</ListItem>
					<ListItem label title="Light">
						{#snippet after()}
							<Radio
								component="div"
								value="light"
								checked={prefs.appearanceTheme === 'light'}
								onChange={() => setThemeAndClose('light')}
							/>
						{/snippet}
					</ListItem>
					<ListItem label title="dark">
						{#snippet after()}
							<Radio
								component="div"
								value="dark"
								checked={prefs.appearanceTheme === 'dark'}
								onChange={() => setThemeAndClose('dark')}
							/>
						{/snippet}
					</ListItem>
				</List>
			</Dialog>
		{/if}
	{/await}
</Page>
