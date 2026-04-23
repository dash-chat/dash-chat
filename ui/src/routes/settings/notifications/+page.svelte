<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
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

	let permissionGranted = $state<boolean | null>(null);
	let toggling = $state(false);

	async function checkPermission() {
		try {
			const { isPermissionGranted } = await import(
				'@tauri-apps/plugin-notification'
			);
			permissionGranted = await isPermissionGranted();
		} catch (e) {
			console.error('Failed to check notification permission:', e);
		}
	}

	async function toggle() {
		if (toggling) return;
		toggling = true;
		try {
			const { isPermissionGranted, requestPermission } = await import(
				'@tauri-apps/plugin-notification'
			);
			const granted = await isPermissionGranted();
			if (!granted) {
				const result = await requestPermission();
				permissionGranted = result === 'granted';
			}
		} catch (e) {
			console.error('Failed to request notification permission:', e);
		} finally {
			toggling = false;
		}
	}

	$effect(() => {
		checkPermission();
	});
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
				<ListItem
					title={m.notifications()}
					data-testid="notifications-toggle"
				>
					{#snippet after()}
						{#if permissionGranted !== null}
							<Toggle
								checked={permissionGranted}
								disabled={toggling || permissionGranted}
								onChange={toggle}
							/>
						{/if}
					{/snippet}
				</ListItem>
			</List>
		</div>
	</div>
</Page>
