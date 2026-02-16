<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import {
		BlockTitle,
		Dialog,
		DialogButton,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
	} from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';

	let showDeleteDialog = $state(false);
	let loading = $state(false);

	async function handleDeleteAccount() {
		loading = true;
		try {
			// TODO: Implement backend command for delete account
			// await invoke('delete_account');
			showToast(m.accountDeleted());
			goto('/');
		} catch (e) {
			console.error(e);
			showToast(m.errorDeleteAccount(), 'error');
		}
		loading = false;
		showDeleteDialog = false;
	}
</script>

<Page>
	<Navbar title={m.account()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/settings')}  data-testid="account-back" />
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.account()}</BlockTitle>
			<List strongIos insetIos>
			<ListItem
				title={m.deleteAccount()}
				link
				chevron={false}
				onClick={() => (showDeleteDialog = true)}
				data-testid="account-delete"
				colors={{
					primaryTextIos: 'text-red-500',
					primaryTextMaterial: 'text-red-500',
				}}
			/>
			</List>
		</div>
	</div>

	<Dialog
		opened={showDeleteDialog}
		onBackdropClick={() => (showDeleteDialog = false)}
	>
		{#snippet title()}
			{m.deleteAccount()}
		{/snippet}
		<span>{m.areYouSureDeleteAccount()}</span>
		{#snippet buttons()}
			<DialogButton onClick={() => (showDeleteDialog = false)} data-testid="account-delete-cancel">
				{m.cancel()}
			</DialogButton>
			<DialogButton strong onClick={handleDeleteAccount} disabled={loading} data-testid="account-delete-confirm">
				{loading ? '...' : m.delete()}
			</DialogButton>
		{/snippet}
	</Dialog>
</Page>
