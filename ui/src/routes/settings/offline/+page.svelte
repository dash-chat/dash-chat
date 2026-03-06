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

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const localMailboxEnabled = useReactivePromise(settingsStore.localMailboxEnabled);
	let toggling = $state(false);

	async function toggle(currentEnabled: boolean) {
		toggling = true;
		try {
			await settingsStore.setLocalMailboxEnabled(!currentEnabled);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			toggling = false;
		}
	}
</script>

<Page>
	<Navbar title={m.offlineFunctionality()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink onClick={() => goto('/settings')} data-testid="offline-back" />
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
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
								disabled={toggling}
								onChange={() => toggle(enabled)}
							/>
						{/await}
					{/snippet}
				</ListItem>
			</List>
			<BlockFooter class="px-4">{m.localMessageServerDescription()}</BlockFooter>
		</div>
	</div>
</Page>
