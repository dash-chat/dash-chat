<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore } from 'dash-chat-stores';
	import { Page, Navbar, NavbarBackLink, Button, Link } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import { page } from '$app/state';
	import FormInput from '$lib/components/form/FormInput.svelte';
	import Form from '$lib/components/form/Form.svelte';
	import Container from '$lib/components/layout_helpers/Container.svelte';
	import EditableAvatar from '$lib/components/profiles/EditableAvatar.svelte';
	import EditingPhotoPage from './EditingPhotoPage.svelte';

	let chatId = page.params.chatId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	const info = useReactivePromise(store.info);
	const infoValue = useReactiveValue(store.info);

	let image = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let description = $state<string>('');

	let initialized = false;

	$effect(() => {
		const i = $infoValue;
		if (i && !initialized) {
			initialized = true;
			image = i.image;
			name = i.name || '';
			description = i.description || '';
		}
	});

	const saveDisabled = $derived(name.trim() === '');

	let editingPhoto = $state(false);
	let imageBeforeEdit = $state<string | undefined>(undefined);

	function startEditPhoto() {
		imageBeforeEdit = image;
		editingPhoto = true;
	}

	function cancelEditPhoto() {
		image = imageBeforeEdit;
		editingPhoto = false;
	}

	function confirmEditPhoto() {
		editingPhoto = false;
	}

	async function save() {
		if (saveDisabled) return;
		await store.setInfo({
			name: name.trim(),
			description: description.trim() || undefined,
			image,
		});
		goto(`/group-chat/${chatId}/info`);
	}
</script>

{#if editingPhoto}
	<EditingPhotoPage
		bind:avatar={image}
		onConfirm={confirmEditPhoto}
		onCancel={cancelEditPhoto}
	/>
{:else}
	<Page>
		<Navbar
			title={m.editGroup()}
			titleClass="opacity1"
			transparent={true}
			rightClass={saveDisabled ? 'ios-right-disabled' : ''}
		>
			{#snippet left()}
				<NavbarBackLink onClick={() => goto(`/group-chat/${chatId}/info`)} />
			{/snippet}
			{#snippet right()}
				{#if isIos}
					<Link data-testid="group-info-edit-save-btn" onClick={save}>
						{m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		{#await $info then info}
			<Container class="pt-2">
				<EditableAvatar
					{image}
					initials={info.name?.slice(0, 2)}
					onEdit={startEditPhoto}
				/>

				<Form>
					<FormInput
						data-testid="group-info-edit-name"
						type="text"
						bind:value={name}
						label={m.name()}
					/>

					<FormInput
						type="textarea"
						inputStyle={{ 'min-height': '2em' }}
						bind:value={description}
						label={m.description()}
					/>
				</Form>
			</Container>

			{#if !isIos}
				<Button
					data-testid="group-info-edit-save-btn"
					onClick={save}
					class="fixed-action-btn"
					rounded
					disabled={saveDisabled}
				>
					{m.save()}
				</Button>
			{/if}
		{/await}
	</Page>
{/if}
