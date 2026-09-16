<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isMobile } from '$lib/utils/environment';
	import { useReactivePromise } from '$lib/stores/use-signal';
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
	import { getContext } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const notificationsEnabled = useReactivePromise(
		settingsStore.notificationsEnabled,
	);

	let toggling = $state(false);

	async function enable() {
		if (toggling) return;
		toggling = true;
		try {
			const { isPermissionGranted, requestPermission } = await import(
				'@tauri-apps/plugin-notification'
			);
			let granted = await isPermissionGranted();
			if (!granted) {
				const result = await requestPermission();
				granted = result === 'granted';
			}
			if (granted) {
				await settingsStore.setNotificationsEnabled(true);
			}
		} catch (e) {
			console.error('Failed to enable notifications:', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			toggling = false;
		}
	}

	async function disable() {
		if (toggling) return;
		toggling = true;
		try {
			await settingsStore.setNotificationsEnabled(false);
		} catch (e) {
			console.error('Failed to disable notifications:', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			toggling = false;
		}
	}
</script>

<Page>
	<Navbar title={m.notifications()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="notifications-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		{#if !isMobile}
			<div class="column center-in-desktop">
				<BlockTitle>{m.messages()}</BlockTitle>
				<List strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListItem title={m.notifications()} data-testid="notifications-toggle">
						{#snippet after()}
							{#await $notificationsEnabled then enabled}
								<Toggle
									checked={enabled}
									disabled={toggling}
									onChange={() => (enabled ? disable() : enable())}
								/>
							{/await}
						{/snippet}
					</ListItem>
				</List>
			</div>
		{/if}
	</div>
</Page>
