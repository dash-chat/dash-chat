<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let {
		agentId,
		name,
		onDone,
	}: {
		agentId: AgentId;
		name: string;
		/** Called after the contact has been reported. */
		onDone?: () => void;
	} = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function confirm(alsoBlock: boolean) {
		try {
			if (alsoBlock) await contactsStore.client.blockContact(agentId);
			await contactsStore.reportContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = alsoBlock
				? m.contactBlockedToast({ name })
				: m.contactReportedToast({ name });
			dialog?.close();
			showToast(toast);
			onDone?.();
		} catch (e) {
			console.error(e);
			dialog?.close();
			showToast(m.contactReportFailedToast(), 'unexpected', e);
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.reportContactTitle({ name })}
	description={m.reportContactDescription()}
	actions={[
		{
			text: m.reportAndBlock(),
			testid: 'report-contact-and-block-confirm',
			onClick: () => confirm(true),
		},
		{
			text: m.report(),
			testid: 'report-contact-confirm',
			onClick: () => confirm(false),
		},
	]}
/>
