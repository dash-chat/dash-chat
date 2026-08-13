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
		useTheme,
	} from 'konsta/svelte';
	import DeleteAccountDialog from '$lib/components/DeleteAccountDialog.svelte';

	const theme = $derived(useTheme());
	let deleteDialog = $state<DeleteAccountDialog>();
</script>

<Page>
	<Navbar title={m.account()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="account-back"
				/>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.account()}</BlockTitle>
			<List strongIos inset={isWideScreen.value || theme === 'ios'}>
				<ListItem
					title={m.deleteAccount()}
					link
					chevron={false}
					onClick={() => deleteDialog?.show()}
					data-testid="account-delete"
					colors={{
						primaryTextIos: 'text-red-500',
						primaryTextMaterial: 'text-red-500',
					}}
				/>
			</List>
		</div>
	</div>

	<DeleteAccountDialog bind:this={deleteDialog} />
</Page>
