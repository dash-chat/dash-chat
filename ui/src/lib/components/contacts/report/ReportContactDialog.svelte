<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
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
		} catch (e) {
			console.error(e);
			opened = false;
			showToast(m.contactReportFailedToast(), 'unexpected', e);
		}
	}
</script>

<Dialog
	{opened}
	onBackdropClick={() => (opened = false)}
	title={m.reportContactTitle({ name })}
>
	<span>{m.reportContactDescription()}</span>
	{#snippet buttons()}
		<DialogButton onClick={() => (opened = false)}>{m.cancel()}</DialogButton>
		<DialogButton
			data-testid="report-contact-and-block-confirm"
			onClick={() => confirm(true)}
		>
			{m.reportAndBlock()}
		</DialogButton>
		<DialogButton
			data-testid="report-contact-confirm"
			onClick={() => confirm(false)}
		>
			{m.report()}
		</DialogButton>
	{/snippet}
</Dialog>
