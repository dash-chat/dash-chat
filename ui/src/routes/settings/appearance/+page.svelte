<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { isIos } from '$lib/utils/environment';
	import {
		type ColorSchemePreference,
		type SettingsStore,
	} from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockTitle,
		Link,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const preference = useReactivePromise(settingsStore.colorSchemePreference);
	const setup = $derived(page.url.searchParams.get('setup') === 'true');

	async function select(scheme: ColorSchemePreference) {
		try {
			await settingsStore.setColorSchemePreference(scheme);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<Page>
	<Navbar title={m.appearance()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="appearance-back"
				/>
			{/if}
		{/snippet}

		{#snippet right()}
			{#if setup && isIos}
				<Link onClick={() => goto('/')} data-testid="appearance-done-btn">
					{m.done()}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	{#await $preference then selected}
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

		{#if setup && !isIos}
			<FixedActionButton onClick={() => goto('/')} testId="appearance-done-btn">
				{m.done()}
			</FixedActionButton>
		{/if}
	{/await}
</Page>
