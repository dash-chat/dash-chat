<script lang="ts">
	import { invokeAfterSetup } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
	import { showToast } from '$lib/utils/toasts';

	let { opened = $bindable() }: { opened: boolean } = $props();

	let loading = $state(false);

	async function confirm() {
		loading = true;
		try {
			// On success the app exits immediately,
			// so no code after this line executes on the happy path.
			await invokeAfterSetup('delete_account');
		} catch (e) {
			console.error(e);
			showToast(m.errorDeleteAccount(), 'error');
			loading = false;
			opened = false;
		}
	}
</script>

{#if isIos}
	<Actions {opened} onBackdropClick={() => (opened = false)}>
		<ActionsGroup class="flex flex-col gap-3 p-2.5">
			<div class="flex flex-col gap-1 px-3.5 py-2 text-start">
				<span class="text-xl text-black dark:text-white">
					{m.deleteAccount()}
				</span>
				<span class="text-black/60 dark:text-white/60">
					{m.areYouSureDeleteAccount()}
				</span>
			</div>
			<ActionButton
				destructive
				onClick={confirm}
				disabled={loading}
				data-testid="account-delete-confirm"
			>
				{loading ? '...' : m.delete()}
			</ActionButton>
			<ActionButton
				onClick={() => (opened = false)}
				data-testid="account-delete-cancel"
			>
				{m.cancel()}
			</ActionButton>
		</ActionsGroup>
	</Actions>
{:else}
	<Dialog
		{opened}
		onBackdropClick={() => (opened = false)}
		title={m.deleteAccount()}
	>
		<span>{m.areYouSureDeleteAccount()}</span>
		{#snippet buttons()}
			<DialogButton
				onClick={() => (opened = false)}
				data-testid="account-delete-cancel"
			>
				{m.cancel()}
			</DialogButton>
			<DialogButton
				onClick={confirm}
				disabled={loading}
				data-testid="account-delete-confirm"
			>
				{loading ? '...' : m.delete()}
			</DialogButton>
		{/snippet}
	</Dialog>
{/if}
