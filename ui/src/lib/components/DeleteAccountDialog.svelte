<script lang="ts">
	import { invokeAfterSetup } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function confirm() {
		try {
			// On success the app exits immediately,
			// so no code after this line executes on the happy path.
			await invokeAfterSetup('delete_account');
		} catch (e) {
			console.error(e);
			showToast(m.errorDeleteAccount(), 'error');
			dialog?.close();
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.deleteAccount()}
	description={m.areYouSureDeleteAccount()}
	actions={[
		{
			text: m.delete(),
			destructive: true,
			testid: 'account-delete-confirm',
			onClick: confirm,
		},
	]}
	cancelTestId="account-delete-cancel"
/>
