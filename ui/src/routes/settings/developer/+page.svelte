<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
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
	import { getContext } from 'svelte';
	import type { SettingsStore } from 'dash-chat-stores';

	const theme = $derived(useTheme());
	const settingsStore: SettingsStore = getContext('settings-store');
	const lanRouterEnabled = useReactivePromise(settingsStore.lanRouterEnabled);
	let togglingLanRouter = $state(false);

	async function toggleLanRouter(currentEnabled: boolean) {
		togglingLanRouter = true;
		try {
			await settingsStore.setLanRouterEnabled(!currentEnabled);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			togglingLanRouter = false;
		}
	}
</script>

<Page data-testid="developer-page">
	<Navbar title={m.developer()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="developer-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.lanRouter()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem
					title={m.enableLanRouter()}
					data-testid="developer-lan-router-toggle"
				>
					{#snippet after()}
						{#await $lanRouterEnabled then enabled}
							<Toggle
								checked={enabled}
								disabled={togglingLanRouter}
								onChange={() => toggleLanRouter(enabled)}
							/>
						{/await}
					{/snippet}
				</ListItem>
			</List>
			<BlockFooter class="px-4">{m.lanRouterDescription()}</BlockFooter>
		</div>
	</div>
</Page>
