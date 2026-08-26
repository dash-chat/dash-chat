<script lang="ts">
	import { invokeAfterSetup } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { setAppShuttingDown } from '$lib/utils/shutdown';

	let { opened = $bindable() }: { opened: boolean } = $props();

	async function confirm() {
		setAppShuttingDown(true);
		try {
			// On success the app exits immediately,
			// so no code after this line executes on the happy path.
			await invokeAfterSetup('delete_account');
			return { success: true as const };
		} catch (e) {
			setAppShuttingDown(false);
			console.error(e);
			return { success: false as const, error: m.errorDeleteAccount() };
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.deleteAccount()}
	description={m.areYouSureDeleteAccount()}
	cancelTestId="account-delete-cancel"
	actions={[
		{
			text: m.delete(),
			destructive: true,
			testid: 'account-delete-confirm',
			onClick: confirm,
		},
	]}
/>
