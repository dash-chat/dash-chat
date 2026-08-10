<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { AgentId, ContactsStore } from 'dash-chat-stores';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import ActionsTitle from '$lib/components/navigation/ActionsTitle.svelte';
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
			await contactsStore.client.blockContact(agentId);
			// Closing can unmount whatever owns `name`, so resolve the toast first.
			const toast = m.contactBlockedToast({ name });
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
		<ActionsGroup
			class="flex flex-col gap-2 !bg-white p-2.5 dark:!bg-neutral-900"
		>
			<ActionsTitle
				title={m.blockContactTitle({ name })}
				subtitle={m.blockContactDescription()}
			/>
			<ActionButton
				destructive
				onClick={confirm}
				data-testid="block-contact-confirm"
			>
				{m.block()}
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
{/if}
