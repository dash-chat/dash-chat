<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import PhotoPicker from './PhotoPicker.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { isIos } from '$lib/utils/environment';
	import {
		Page,
		Button,
		ListInput,
		List,
		useTheme,
		Link,
		Navbar,
		Card,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCamera, mdiAccount } from '@mdi/js';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string | undefined>(undefined);
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);
	let showPicker = $state(false);
	let pickerAvatar = $state<string | undefined>(undefined);

	function openPicker() {
		pickerAvatar = avatar;
		showPicker = true;
	}

	function closePicker() {
		showPicker = false;
	}

	function selectAvatar() {
		avatar = pickerAvatar;
		showPicker = false;
	}

	async function setProfile() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about: undefined,
			});
		} catch (e) {
			console.error(e);
			const error = e as Error;
			switch (error.kind) {
				case 'AuthorOperation':
					showToast(m.errorSetProfile(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}
	const theme = $derived(useTheme());
	const pickerHasChanges = $derived(pickerAvatar !== avatar);
	let textEditorOpen = $state(false);
</script>

<Page>
	{#if showPicker}
		<div class="column" style="flex: 1; overflow-y: auto">
			<PhotoPicker
				bind:avatar={pickerAvatar}
				bind:isTextEditorOpen={textEditorOpen}
				onClose={closePicker}
				onSave={selectAvatar}
				saveLabel={m.save()}
				saveDisabled={!pickerHasChanges}
			/>
		</div>

		{#if !textEditorOpen && !isIos}
			<Button
				rounded
				tonal
				disabled={!pickerHasChanges}
				onClick={selectAvatar}
				class="fixed-action-btn"
			>
				{m.save()}
			</Button>
		{/if}
	{:else}
		<Navbar
			title={m.setProfile()}
			titleClass="opacity1"
			transparent={true}
			rightClass={name === undefined || name === '' ? 'ios-right-disabled' : ''}
		>
			{#snippet right()}
				{#if isIos}
					<Link onClick={setProfile} data-testid="create-profile-create-link">
						{m.create()}
					</Link>
				{/if}
			{/snippet}
			{#snippet subtitle()}{/snippet}
		</Navbar>

		<div class="column" style="flex: 1; overflow-y: auto">
			<div class="center-in-desktop column gap-2 p-2">
				<span class="quiet px-4 py-2" class:pt-4={theme === 'ios'}>
					{m.setProfileExplanation()}
				</span>

				<button
					type="button"
					class="avatar-btn"
					style="position: relative; align-self: center; cursor: pointer"
					onclick={openPicker}
				>
					{#if avatar}
						<wa-avatar
							image={avatar}
							alt="Avatar"
							shape="circle"
							style="--size: 56px"
						></wa-avatar>
					{:else}
						<Button
							rounded
							style="border-radius: 50%; height: 56px; width: 56px"
						>
							<wa-icon
								src={wrapPathInSvg(mdiAccount)}
								label={m.addAvatarImage()}
							></wa-icon>
						</Button>
					{/if}
					<Card
						class="icon-only-card"
						raised
						style="position: absolute; pointer-events: none; bottom: -6px; right: -6px; z-index: 10"
					>
						<wa-icon src={wrapPathInSvg(mdiCamera)}></wa-icon>
					</Card>
				</button>

				<List inset={isWideScreen.value || theme === 'ios'} strongIos>
					<ListInput
						type="text"
						onInput={e => (name = e.target.value)}
						placeholder={m.nameMandatory()}
						data-testid="create-profile-name"
					></ListInput>
					<ListInput
						type="text"
						onInput={e => (surname = e.target.value)}
						placeholder={m.surnameOptional()}
						data-testid="create-profile-surname"
					></ListInput>
				</List>
			</div>
		</div>

		{#if !isIos}
			<Button
				onClick={setProfile}
				class="fixed-action-btn"
				rounded
				disabled={name === undefined || name === ''}
				data-testid="create-profile-create-btn"
			>
				{m.create()}
			</Button>
		{/if}
	{/if}
</Page>

<style>
	.avatar-btn {
		background: transparent;
		border: none;
		padding: 0;
	}
</style>
