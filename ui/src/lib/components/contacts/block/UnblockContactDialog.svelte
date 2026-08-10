<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
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
			await contactsStore.client.unblockContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = m.contactUnblockedToast({ name });
			opened = false;
			showToast(toast);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

{#if isIos}
	<Actions {opened} onBackdropClick={() => (opened = false)}>
		<ActionsGroup class="flex flex-col gap-3 p-2.5">
			<div class="flex flex-col gap-1 px-3.5 py-2 text-start">
				<span class="text-xl text-black dark:text-white">
					{m.unblockContactTitle({ name })}
				</span>
				<span class="text-black/60 dark:text-white/60">
					{m.unblockContactDescription()}
				</span>
			</div>
			<ActionButton onClick={confirm} data-testid="unblock-contact-confirm">
				{m.unblock()}
			</ActionButton>
			<ActionButton onClick={() => (opened = false)}>
				{m.cancel()}
			</ActionButton>
		</ActionsGroup>
	</Actions>
{:else}
	<Dialog
		{opened}
		onBackdropClick={() => (opened = false)}
		title={m.unblockContactTitle({ name })}
	>
		<span>{m.unblockContactDescription()}</span>
		{#snippet buttons()}
			<DialogButton onClick={() => (opened = false)}>{m.cancel()}</DialogButton>
			<DialogButton data-testid="unblock-contact-confirm" onClick={confirm}>
				{m.unblock()}
			</DialogButton>
		{/snippet}
	</Dialog>
{/if}
