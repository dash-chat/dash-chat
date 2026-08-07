<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, Error, SettingsStore } from 'dash-chat-stores';
	import AvatarPicker from './AvatarPicker.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { isIos, isMobile } from '$lib/utils/environment';
	import {
		Page,
		Button,
		ListInput,
		List,
		useTheme,
		Link,
		Navbar,
		NavbarBackLink,
		Card,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCamera, mdiAccount } from '@mdi/js';
	import Avatar from './Avatar.svelte';

	let { onBack }: { onBack?: () => void } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const settingsStore: SettingsStore = getContext('settings-store');
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

	async function requestNotificationPermission() {
		if (!isMobile) return;
		try {
			const { isPermissionGranted, requestPermission } = await import(
				'@tauri-apps/plugin-notification'
			);
			let granted = await isPermissionGranted();
			if (!granted) {
				const result = await requestPermission();
				granted = result === 'granted';
			}
			if (granted) {
				await settingsStore.setNotificationsEnabled(true);
			}
		} catch (e) {
			console.error('Failed to setup push notifications:', e);
		}
	}

	async function setProfile() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about: undefined,
			});
			await requestNotificationPermission();
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

	const avatarSize = 100;

	const NAME_INPUT_ID = 'create-profile-name-input';

	// Reads no reactive state, so it focuses once on mount. The `autofocus`
	// attribute is unreliable here: the page is mounted after load, when the
	// step machine swaps it in.
	$effect(() => {
		document.getElementById(NAME_INPUT_ID)?.focus();
	});
</script>

<Page class="pb-keyboard-safe">
	{#if showPicker}
		<AvatarPicker
			bind:avatar={pickerAvatar}
			bind:inModalState={textEditorOpen}
			onClose={closePicker}
			onSave={selectAvatar}
			saveLabel={m.save()}
			saveDisabled={!pickerHasChanges}
		/>

		{#if !textEditorOpen && !isIos}
			<FixedActionButton
				tonal
				disabled={!pickerHasChanges}
				onClick={selectAvatar}
			>
				{m.save()}
			</FixedActionButton>
		{/if}
	{:else}
		<Navbar
			title={m.setProfile()}
			titleClass="opacity1"
			transparent={true}
			rightClass={name === undefined || name === '' ? 'ios-right-disabled' : ''}
		>
			{#snippet left()}
				{#if onBack}
					<NavbarBackLink onClick={onBack} data-testid="create-profile-back" />
				{/if}
			{/snippet}

			{#snippet right()}
				{#if isIos}
					<Link onClick={setProfile} data-testid="create-profile-create-btn">
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
					style="height: 100%; position: relative; align-self: center; cursor: pointer"
					onclick={openPicker}
				>
					{#if avatar}
						<Avatar image={avatar} alt="Avatar" size={avatarSize} />
					{:else}
						<Button
							rounded
							style="border-radius: 50%; height: {avatarSize}px; width: {avatarSize}px"
						>
							<wa-icon
								src={wrapPathInSvg(mdiAccount)}
								label={m.addAvatarImage()}
								style="font-size: {avatarSize * 0.6}px;"
							></wa-icon>
						</Button>
					{/if}
					<Card
						class="icon-only-card"
						raised
						style="position: absolute; pointer-events: none; bottom: 0px; inset-inline-end: -6px; z-index: 10;"
					>
						<wa-icon src={wrapPathInSvg(mdiCamera)}></wa-icon>
					</Card>
				</button>

				<List inset={isWideScreen.value || theme === 'ios'} strongIos>
					<ListInput
						type="text"
						value={name ?? ''}
						onInput={e => (name = e.target.value)}
						placeholder={m.nameRequired()}
						inputId={NAME_INPUT_ID}
						data-testid="create-profile-name"
					></ListInput>
					<ListInput
						type="text"
						value={surname ?? ''}
						onInput={e => (surname = e.target.value)}
						placeholder={m.surnameOptional()}
						data-testid="create-profile-surname"
					></ListInput>
				</List>
			</div>
		</div>

		{#if !isIos}
			<FixedActionButton
				onClick={setProfile}
				disabled={name === undefined || name === ''}
				testId="create-profile-create-btn"
			>
				{m.create()}
			</FixedActionButton>
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
