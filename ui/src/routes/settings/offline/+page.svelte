<script lang="ts">
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { type SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockFooter,
		BlockTitle,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Toggle,
		useTheme,
	} from 'konsta/svelte';
	import { isAndroid, isMobile } from '$lib/utils/environment';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const localMailboxEnabled = useReactivePromise(
		settingsStore.localMailboxEnabled,
	);
	const backgroundModeEnabled = useReactivePromise(
		settingsStore.backgroundModeEnabled,
	);
	let togglingLocalMailbox = $state(false);
	let togglingBackgroundMode = $state(false);

	async function toggleLocalMailbox(currentEnabled: boolean) {
		togglingLocalMailbox = true;
		try {
			await settingsStore.setLocalMailboxEnabled(!currentEnabled);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			togglingLocalMailbox = false;
		}
	}

	async function toggleBackgroundMode(currentEnabled: boolean) {
		togglingBackgroundMode = true;
		try {
			await settingsStore.setBackgroundModeEnabled(!currentEnabled);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			togglingBackgroundMode = false;
		}
	}
</script>

<Page>
	<Navbar
		title={m.offlineFunctionality()}
		titleClass="opacity1"
		transparent={true}
	>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="offline-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		{#if !isMobile}
			<div class="column center-in-desktop">
				<BlockTitle>{m.localMessageServer()}</BlockTitle>
				<List strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListItem
						title={m.enableLocalMessageServer()}
						data-testid="offline-local-mailbox-toggle"
					>
						{#snippet after()}
							{#await $localMailboxEnabled then enabled}
								<Toggle
									checked={enabled}
									disabled={togglingLocalMailbox}
									onChange={() => toggleLocalMailbox(enabled)}
								/>
							{/await}
						{/snippet}
					</ListItem>
				</List>
				<BlockFooter class="px-4"
					>{m.localMessageServerDescription()}</BlockFooter
				>
			</div>
		{/if}

		{#if isAndroid}
			<div class="column center-in-desktop">
				<BlockTitle>{m.backgroundMode()}</BlockTitle>
				<List strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListItem
						title={m.startBackgroundMode()}
						data-testid="offline-background-mode-toggle"
					>
						{#snippet after()}
							{#await $backgroundModeEnabled then enabled}
								<Toggle
									checked={enabled}
									disabled={togglingBackgroundMode}
									onChange={() => toggleBackgroundMode(enabled)}
								/>
							{/await}
						{/snippet}
					</ListItem>
				</List>
				<BlockFooter class="px-4">{m.backgroundModeDescription()}</BlockFooter>
			</div>
		{/if}
	</div>
</Page>
