<script lang="ts">
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { type SettingsStore, type ColorScheme } from 'dash-chat-stores';
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
	const settingsStore: SettingsStore = getContext('settings-store');
	const colorScheme = useReactivePromise(settingsStore.colorScheme);

	async function select(scheme: ColorScheme) {
		try {
			await settingsStore.setColorScheme(scheme);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<Page>
	<Navbar title={m.appearance()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink onClick={() => goto('/settings')} data-testid="appearance-back" />
			{/if}
		{/snippet}
	</Navbar>

	{#await $colorScheme then selected}
		<div class="column" style="flex: 1">
			<div class="column center-in-desktop">
				<BlockTitle>{m.colorScheme()}</BlockTitle>
				<List strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListItem
						title={m.lightMode()}
						link
						chevron={false}
						onClick={() => select('light')}
						data-testid="appearance-light"
					>
						{#snippet after()}
							{#if selected === 'light'}
								<span class="text-brand-primary">✓</span>
							{/if}
						{/snippet}
					</ListItem>
					<ListItem
						title={m.darkMode()}
						link
						chevron={false}
						onClick={() => select('dark')}
						data-testid="appearance-dark"
					>
						{#snippet after()}
							{#if selected === 'dark'}
								<span class="text-brand-primary">✓</span>
							{/if}
						{/snippet}
					</ListItem>
					<ListItem
						title={m.systemDefault()}
						link
						chevron={false}
						onClick={() => select('system')}
						data-testid="appearance-system"
					>
						{#snippet after()}
							{#if selected === 'system'}
								<span class="text-brand-primary">✓</span>
							{/if}
						{/snippet}
					</ListItem>
				</List>
			</div>
		</div>
	{/await}
</Page>
