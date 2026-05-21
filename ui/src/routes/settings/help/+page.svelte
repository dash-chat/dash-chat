<script lang="ts">
	import { goto } from '$app/navigation';
	import { getVersion } from '@tauri-apps/api/app';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { developerFeatures } from '$lib/stores/developer-features.svelte';
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
				{#await versionPromise then version}
					<ListItem
						title={m.version()}
						after={version}
						data-testid="help-version"
					/>
				{/await}
				<ListItem
					title={m.developerFeatures()}
					data-testid="help-developer-features-toggle"
				>
					{#snippet after()}
						<Toggle
							checked={developerFeatures.enabled}
							onChange={() => developerFeatures.toggle()}
						/>
					{/snippet}
				</ListItem>
			</List>
		</div>
	</div>
</Page>
