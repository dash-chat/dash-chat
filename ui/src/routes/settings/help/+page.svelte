<script lang="ts">
	import { goto } from '$app/navigation';
	import { getVersion } from '@tauri-apps/api/app';
	import { m } from '$lib/paraglide/messages.js';
	import { developerMode } from '$lib/stores/developer-mode.svelte';
	import { offlineMode } from '$lib/stores/offline-mode.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { previewFeatures } from '$lib/stores/preview-features.svelte';
	import { showToast } from '$lib/utils/toasts';
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

	const theme = $derived(useTheme());

	const versionPromise = getVersion();

	const UNLOCK_TAP_COUNT = 7;
	const MAX_TAP_GAP_MS = 300;

	let tapCount = 0;
	let lastTapAt = 0;

	function onVersionTap() {
		if (developerMode.unlocked) return;
		const now = Date.now();
		tapCount = now - lastTapAt < MAX_TAP_GAP_MS ? tapCount + 1 : 1;
		lastTapAt = now;
		if (tapCount < UNLOCK_TAP_COUNT) return;
		tapCount = 0;
		developerMode.unlock();
		showToast(m.developerModeEnabled());
	}
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
				<!-- Disabling the background service for now -->
				{#if false}
					<ListItem
						title={m.startOfflineMode()}
						data-testid="help-start-offline-mode"
					>
						{#snippet after()}
							<Toggle
								checked={offlineMode.enabled}
								onChange={() => offlineMode.toggle()}
								data-testid="help-start-offline-mode-switch"
							/>
						{/snippet}
					</ListItem>
				{/if}
				{#await versionPromise then version}
					<ListItem
						title={m.version()}
						after={version}
						onClick={onVersionTap}
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
