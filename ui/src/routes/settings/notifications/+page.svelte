<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
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
	import { getContext, onMount } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import {
		ensureNotificationPermission,
		notificationPermissionGranted,
	} from '$lib/utils/notifications';
	import PermissionSettingsSheet from '$lib/components/PermissionSettingsSheet.svelte';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const notificationsEnabled = useReactivePromise(
		settingsStore.notificationsEnabled,
	);

	let toggling = $state(false);
	let refusedEnables = $state(0);
	let showSettingsSheet = $state(false);
	// The setting keeps the user's choice even when the permission is revoked
	// from the device settings, so the toggle shows both.
	let permissionGranted = $state<boolean | undefined>();

	onMount(async () => {
		permissionGranted = (await notificationPermissionGranted()) === true;
	});

	async function enable() {
		if (toggling) return;
		toggling = true;
		try {
			if (await ensureNotificationPermission()) {
				permissionGranted = true;
				await settingsStore.setNotificationsEnabled(true);
			} else {
				refusedEnables += 1;
				if ((await notificationPermissionGranted()) === false) {
					showSettingsSheet = true;
				} else {
					showToast(m.notificationsPermissionDenied(), 'error');
				}
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
		<div class="column center-in-desktop">
			<BlockTitle>{m.messages()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem title={m.notifications()} data-testid="notifications-toggle">
					{#snippet after()}
						{#await $notificationsEnabled then enabled}
							{#if permissionGranted !== undefined}
								<!-- Konsta's Toggle keeps the checked state its own click set, so a
								     refused enable remounts it to show the stored value again. -->
								{#key refusedEnables}
									<Toggle
										checked={enabled && permissionGranted}
										disabled={toggling}
										onChange={() =>
											enabled && permissionGranted ? disable() : enable()}
									/>
								{/key}
							{/if}
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
