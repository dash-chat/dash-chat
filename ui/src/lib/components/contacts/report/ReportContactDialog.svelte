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
		onDone,
	}: {
		opened: boolean;
		agentId: AgentId;
		name: string;
		/** Called after the contact has been reported. */
		onDone?: () => void;
	} = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	async function confirm(alsoBlock: boolean) {
		try {
			if (alsoBlock) await contactsStore.client.blockContact(agentId);
			await contactsStore.reportContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = alsoBlock
				? m.contactBlockedToast({ name })
				: m.contactReportedToast({ name });
			opened = false;
			showToast(toast);
			onDone?.();
			return { success: true as const };
		} catch (e) {
			console.error(e);
			return {
				success: false as const,
				error: m.contactReportFailedToast(),
				cause: e,
			};
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.reportContactTitle({ name })}
	description={m.reportContactDescription()}
	actions={[
		{
			text: m.reportAndBlock(),
			testid: 'report-contact-and-block-confirm',
			destructive: true,
			onClick: () => confirm(true),
		},
		{
			text: m.report(),
			testid: 'report-contact-confirm',
			destructive: true,
			onClick: () => confirm(false),
		},
	]}
/>
