<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import {
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
	import { getContext } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import {
		type NotificationPermissionState,
		getNotificationPermission,
		requestNotificationPermission,
	} from '$lib/utils/notifications';
	import PermissionSettingsSheet from '$lib/components/PermissionSettingsSheet.svelte';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const notificationsEnabled = useReactivePromise(
		settingsStore.notificationsEnabled,
	);

	let toggling = $state(false);
	let permission = $state<NotificationPermissionState | undefined>(undefined);
	let showSettingsSheet = $state(false);

	function refreshPermission() {
		getNotificationPermission()
			.then(state => (permission = state))
			.catch(e => console.error('Failed to read notification permission:', e));
	}

	async function turnOn() {
		if (permission === 'denied') {
			showSettingsSheet = true;
			return;
		}
		try {
			permission = await requestNotificationPermission();
		} catch (e) {
			console.error('Failed to read notification permission:', e);
		}
	}

	$effect(() => {
		refreshPermission();
		const onVisibilityChange = () => {
			if (document.visibilityState === 'visible') refreshPermission();
		};
		document.addEventListener('visibilitychange', onVisibilityChange);
		return () =>
			document.removeEventListener('visibilitychange', onVisibilityChange);
	});

	async function setEnabled(enabled: boolean) {
		if (toggling) return;
		toggling = true;
		try {
			await settingsStore.setNotificationsEnabled(enabled);
		} catch (e) {
			console.error('Failed to update notifications setting:', e);
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
		<div class="column center-in-desktop">
			{#if permission === 'prompt' || permission === 'denied'}
				<div class="px-4 pt-2">
					<div
						class="flex flex-col gap-1 rounded-xl bg-brand-primary/10 px-4 pt-4 pb-2"
						data-testid="notifications-permission-banner"
					>
						<span class="text-sm" style="color: var(--k-text-color)">
							{m.notificationsPermissionBanner()}
						</span>
						<div class="flex justify-end">
							<Button
								inline
								clear
								onClick={turnOn}
								data-testid="notifications-permission-turn-on"
							>
								{m.notificationsTurnOn()}
							</Button>
						</div>
					</div>
				</div>
			{/if}
			<BlockTitle>{m.messages()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem title={m.notifications()} data-testid="notifications-toggle">
					{#snippet after()}
						{#await $notificationsEnabled then enabled}
							<Toggle
								checked={enabled && permission === 'granted'}
								disabled={toggling || permission !== 'granted'}
								onChange={() => setEnabled(!enabled)}
							/>
						{/await}
					{/snippet}
				</ListItem>
			</List>
		</div>
	</div>
</Page>

<PermissionSettingsSheet
	bind:opened={showSettingsSheet}
	title={m.notificationsSettingsTitle()}
	subtitle={m.notificationsSettingsSubtitle()}
	steps={[
		m.notificationsSettingsStep1(),
		m.notificationsSettingsStep2(),
		m.notificationsSettingsStep3(),
	]}
/>
