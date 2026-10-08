<script module lang="ts">
	// Whether the user has already refused the OS prompt once this app run. The
	// prompt returns 'denied' both when it was shown-and-declined and when it's
	// permanently suppressed, so this is how we tell them apart.
	let deniedOnce = false;
</script>

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
	import { getContext } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';
	import { showToast } from '$lib/utils/toasts';
	import { ensureNotificationPermission } from '$lib/utils/notifications';
	import PermissionSettingsSheet from '$lib/components/PermissionSettingsSheet.svelte';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const notificationsEnabled = useReactivePromise(
		settingsStore.notificationsEnabled,
	);

	let toggling = $state(false);
	let refusedEnables = $state(0);
	let showSettingsSheet = $state(false);

	async function enable() {
		if (toggling) return;
		toggling = true;
		try {
			if (await ensureNotificationPermission()) {
				deniedOnce = false;
				await settingsStore.setNotificationsEnabled(true);
			} else {
				refusedEnables += 1;
				// On the first refusal the OS dialog was shown and the next attempt
				// re-prompts, so a toast is enough. Once permanently denied the
				// dialog no longer shows, so guide the user to the app's settings.
				if (deniedOnce) {
					showSettingsSheet = true;
				} else {
					deniedOnce = true;
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
							<!-- Konsta's Toggle keeps the checked state its own click set, so a
							     refused enable remounts it to show the stored value again. -->
							{#key refusedEnables}
								<Toggle
									checked={enabled}
									disabled={toggling}
									onChange={() => (enabled ? disable() : enable())}
								/>
							{/key}
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
