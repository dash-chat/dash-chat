<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let {
		agentId,
		name,
	}: {
		agentId: AgentId;
		name: string;
	} = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function confirm() {
		try {
			await contactsStore.client.unblockContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = m.contactUnblockedToast({ name });
			dialog?.close();
			showToast(toast);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.unblockContactTitle({ name })}
	description={m.unblockContactDescription()}
	actions={[
		{ text: m.unblock(), testid: 'unblock-contact-confirm', onClick: confirm },
	]}
/>
