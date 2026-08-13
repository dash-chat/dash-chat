<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type {
		AddContactError,
		ContactRequest,
		ContactsStore,
	} from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let { contactRequest }: { contactRequest: ContactRequest } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function accept() {
		dialog?.close();
		try {
			await contactsStore.client.acceptContact(contactRequest.agentId);
			showToast(m.contactAccepted());
		} catch (e) {
			console.error(e);
			const error = e as AddContactError;
			switch (error.kind) {
				case 'ProfileNotCreated':
					showToast(m.errorAddContactProfileRequired(), 'error');
					break;
				case 'InitializeTopic':
				case 'AuthorOperation':
				case 'CreateQrCode':
				case 'CreateDirectChat':
				case 'StoreContact':
					showToast(m.errorAddContact(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.acceptRequestTitle()}
	description={m.acceptRequestDescription()}
	actions={[
		{ text: m.accept(), testid: 'direct-chat-accept-confirm', onClick: accept },
	]}
/>
