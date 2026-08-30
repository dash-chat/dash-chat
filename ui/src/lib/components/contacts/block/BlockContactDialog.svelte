<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let {
		opened = $bindable(),
		agentId,
		name,
	}: {
		opened: boolean;
		agentId: AgentId;
		name: string;
	} = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	async function confirm() {
		try {
			await contactsStore.client.blockContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = m.contactBlockedToast({ name });
			opened = false;
			showToast(toast);
			return { success: true as const };
		} catch (e) {
			console.error(e);
			return { success: false as const, error: m.errorUnexpected(), cause: e };
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.blockContactTitle({ name })}
	description={m.blockContactDescription()}
	actions={[
		{
			text: m.block(),
			destructive: true,
			testid: 'block-contact-confirm',
			onClick: confirm,
		},
	]}
/>
