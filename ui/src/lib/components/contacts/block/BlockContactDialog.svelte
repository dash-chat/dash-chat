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
	}: {
		opened: boolean;
		agentId: AgentId;
		name: string;
	} = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	async function confirm() {
		try {
			await contactsStore.client.blockContact(agentId);
			opened = false;
			showToast(m.contactBlockedToast({ name }));
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<Dialog
	{opened}
	onBackdropClick={() => (opened = false)}
	title={m.blockContactTitle({ name })}
>
	<span>{m.blockContactDescription()}</span>
	{#snippet buttons()}
		<DialogButton onClick={() => (opened = false)}>{m.cancel()}</DialogButton>
		<DialogButton data-testid="block-contact-confirm" onClick={confirm}>
			{m.block()}
		</DialogButton>
	{/snippet}
</Dialog>
