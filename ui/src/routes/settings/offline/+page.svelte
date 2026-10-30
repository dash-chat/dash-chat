<script lang="ts">
	import { goBack } from '$lib/utils/navigation';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { type SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/links';
	import {
		BlockFooter,
		BlockTitle,
		Button,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Toggle,
		useTheme,
	} from 'konsta/svelte';
	import { isAndroid, isMac, isMobile } from '$lib/utils/environment';
	import WarningIcon from '$lib/components/WarningIcon.svelte';
	import BorderedBox from '$lib/components/BorderedBox.svelte';

	// macOS 15 opens Privacy & Security here, one click short of Local Network.
	const LOCAL_NETWORK_SETTINGS_URL =
		'x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const localMailboxEnabled = useReactivePromise(
		settingsStore.localMailboxEnabled,
	);
	const localHubBlocked = useReactiveValue(settingsStore.localHubBlocked);
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

	async function openLocalNetworkSettings() {
		try {
			await openExternalUrl(LOCAL_NETWORK_SETTINGS_URL);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
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
					onClick={() => goBack('/settings')}
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
				{#if $localHubBlocked === true}
					<div class="px-4 pt-4">
						<BorderedBox data-testid="offline-local-mailbox-lan-blocked">
							<div class="flex gap-3 text-sm">
								<WarningIcon size={24} color="text-red-500" class="shrink-0" />
								<div class="flex flex-col gap-2">
									<span class="font-semibold">{m.localHubCantBeFound()}</span>
									{#if isMac}
										<span>{m.localHubLanBlockedMacosIntro()}</span>
										<ol class="flex list-decimal flex-col gap-2 ps-5">
											<li>
												<div
													class="flex flex-wrap items-center gap-x-3 gap-y-2"
												>
													<span class="min-w-40 flex-1"
														>{m.localHubLanBlockedMacosStep1()}</span
													>
													<Button
														inline
														tonal
														class="shrink-0 whitespace-nowrap"
														onClick={openLocalNetworkSettings}
														data-testid="offline-local-mailbox-open-settings"
														>{m.openSystemSettings()}</Button
													>
												</div>
											</li>
											<li>{m.localHubLanBlockedMacosStep2()}</li>
											<li>{m.localHubLanBlockedMacosStep3()}</li>
										</ol>
										<span class="opacity-70"
											>{m.localHubLanBlockedMacosRecurs()}</span
										>
									{:else}
										<span>{m.localHubLanBlockedOther()}</span>
									{/if}
								</div>
							</div>
						</BorderedBox>
					</div>
				{/if}
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
