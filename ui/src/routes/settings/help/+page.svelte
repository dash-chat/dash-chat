<script lang="ts">
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { previewFeatures } from '$lib/stores/preview-features.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import type { SettingsStore } from 'dash-chat-stores';
	import {
		BlockTitle,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Toggle,
		useTheme,
	} from 'konsta/svelte';
	import { isAndroid } from '$lib/utils/environment';

	const theme = $derived(useTheme());

	const settingsStore = getContext<SettingsStore>('settings-store');
	const backgroundModeEnabled = useReactivePromise(settingsStore.backgroundModeEnabled);

	const versionPromise = getVersion();
</script>

<Page>
	<Navbar title={m.help()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="help-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.help()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem
					link
					chevron={false}
					linkProps={{ href: '/settings/help/contact-us' }}
					title={m.contactUs()}
					data-testid="help-contact-us"
				/>
				{#if true || isAndroid}
					<ListItem
						title={m.startOfflineMode()}
						data-testid="help-start-offline-mode"
					>
						{#snippet after()}
							{#await $backgroundModeEnabled then enabled}
								<Toggle
									checked={enabled}
									onChange={() => settingsStore.setBackgroundModeEnabled(!enabled)}
									data-testid="help-start-offline-mode-switch"
								/>
							{/await}
						{/snippet}
					</ListItem>
				{/if}
				{#await versionPromise then version}
					<ListItem
						title={m.version()}
						after={version}
						data-testid="help-version"
					/>
				{/await}
				<!-- Removing for now because we don't have any preview feature -->
				{#if false}
					<ListItem
						title={m.previewFeatures()}
						data-testid="help-preview-features-toggle"
					>
						{#snippet after()}
							<Toggle
								checked={previewFeatures.enabled}
								onChange={() => previewFeatures.toggle()}
							/>
						{/snippet}
					</ListItem>
				{/if}
			</List>
		</div>
	</div>
</Page>
