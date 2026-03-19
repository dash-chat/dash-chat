<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import {
		decodeContactCode,
		encodeContactCode,
		fullName,
		toPromise,
		type ContactsStore,
		type SettingsStore,
	} from 'dash-chat-stores';
	import type { AddContactError } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';

	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';
	import { scanQrFromImage } from '$lib/utils/qrcode';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		ListInput,
		List,
		Preloader,
		Button,
		useTheme,
		ToolbarPane,
		TabbarLink,
		Tabbar,
	} from 'konsta/svelte';
	import { goto } from '$app/navigation';
	import { showToast } from '$lib/utils/toasts';
	import { saveQrCode, shareQrCode } from '$lib/utils/save-qr-code';
	import SelectColor from './SelectColor.svelte';
	import MyQrCodeCard from '$lib/components/contacts/MyQrCodeCard.svelte';
	import QrActionButtons from '$lib/components/contacts/QrActionButtons.svelte';
	import QrCodeScanner from '$lib/components/contacts/QrCodeScanner.svelte';

	type TabName = 'code' | 'scan';

	let { showBack = true }: { showBack?: boolean } = $props();

	const theme = $derived(useTheme());

	const contactsStore: ContactsStore = getContext('contacts-store');
	const settingsStore: SettingsStore = getContext('settings-store');

	let myCode = contactsStore.client.createContactCode().then(encodeContactCode);

	let tab = $state<TabName>('code');
	let scannerRef: QrCodeScanner | null = $state(null);

	async function receiveCode(code: string) {
		try {
			const contactCode = decodeContactCode(code);

			const myCodeString = await myCode;

			if (code === myCodeString) {
				showToast(m.cantAddYourselfAsContact(), 'error');
				return;
			}

			// Don't send a contact request if they're already in your contacts
			//
			// Uncommenting this would mean that if the contact rejected your contact request
			// there is no way to resend the contact request
			//
			// const contacts = await toPromise(contactsStore.contactsAgentIds);
			//
			// if (contacts.includes(contactCode.agent_id)) {
			// 	showToast(m.contactAlreadyExists());
			// 	return;
			// }

			await contactsStore.client.addContact(contactCode);
			showToast(m.contactAccepted());

			goto(`/direct-chats/${contactCode.agent_id}`);
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

	const qrColor = useReactivePromise(settingsStore.qrColor);
	let colorPickerOpen = $state(false);
	let colorForPicker = $state('#007aff');

	async function getMyName(): Promise<string> {
		const profile = await toPromise(contactsStore.myProfile);
		return profile ? fullName(profile) : '';
	}

	async function shareCode(code: string) {
		try {
			const name = await getMyName();
			const color = await toPromise(settingsStore.qrColor);
			await shareQrCode(code, color, name);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	async function openColorPicker() {
		colorForPicker = await toPromise(settingsStore.qrColor);
		colorPickerOpen = true;
	}

	async function saveCode(code: string, color: string) {
		try {
			const name = await getMyName();
			await saveQrCode(code, color ?? '#007aff', name);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	let imageFilePicker: HTMLInputElement;

	async function switchTab(nextTab: TabName) {
		if (nextTab === tab) return;

		if (tab === 'scan' && nextTab !== 'scan' && scannerRef) {
			await scannerRef.cancelScanner();
		}

		tab = nextTab;
	}

	async function onImageSelected() {
		if (!imageFilePicker.files || !imageFilePicker.files[0]) return;
		try {
			const code = await scanQrFromImage(imageFilePicker.files[0]);
			await receiveCode(code);
		} catch (e) {
			console.error(e);
			showToast(m.errorNoQrCodeInImage(), 'error');
		} finally {
			imageFilePicker.value = '';
		}
	}

	function onScannerRequestPickFile() {
		imageFilePicker.click();
	}
</script>

<input
	type="file"
	accept="image/*"
	bind:this={imageFilePicker}
	style="display: none"
	onchange={onImageSelected}
/>

{#if colorPickerOpen}
	{#await myCode then code}
		<SelectColor
			{code}
			qrColor={colorForPicker}
			onClose={() => (colorPickerOpen = false)}
		/>
	{/await}
{:else}
	<Page
		class={tab === 'scan' ? 'transparent' : ''}
		style="display: flex; flex-direction: column"
	>
		<Navbar
			centerTitle={isMobile || theme === 'ios'}
			titleClass="opacity1"
			transparent={true}
			style={tab === 'scan' && theme === 'material'
				? 'background-color: var(--background-color)'
				: ''}
		>
			{#snippet left()}
				{#if showBack}
					<NavbarBackLink
						data-testid="add-contact-back"
						onClick={() => {
							window.history.back();
						}}
					/>
				{/if}
			{/snippet}

			{#snippet title()}
				{#if isMobile}
					{#if theme === 'material'}
						<div
							class="row gap-2"
							style="align-items: center; justify-content: center"
						>
							<Button
								class="w-24"
								small
								rounded
								tonal={tab !== 'code'}
								onClick={() => void switchTab('code')}
								data-testid="add-contact-code-tab"
								>{m.code()}
							</Button>

							<Button
								class="w-24"
								small
								rounded
								tonal={tab !== 'scan'}
								onClick={() => void switchTab('scan')}
								data-testid="add-contact-scan-tab"
								>{m.scan()}
							</Button>
						</div>
					{:else}
						<Tabbar
							labels={true}
							class="transparent"
							style="margin-top: env(safe-area-inset-top); z-index: -1;"
						>
							<ToolbarPane>
								<TabbarLink
									active={tab === 'code'}
									onclick={() => void switchTab('code')}
									label={m.code()}
									data-testid="add-contact-code-tab"
								/>
								<TabbarLink
									active={tab !== 'code'}
									onclick={() => void switchTab('scan')}
									label={m.scan()}
									data-testid="add-contact-scan-tab"
								/>
							</ToolbarPane>
						</Tabbar>
					{/if}
				{:else}
					{m.addContact()}
				{/if}
			{/snippet}
		</Navbar>

		{#if tab === 'code'}
			{#await myCode}
				<div
					class="column"
					style="height: 100%; align-items: center; justify-content: center"
				>
					<Preloader />
				</div>
			{:then code}
				{#await $qrColor then color}
					<div class="column" style="flex:1">
						<div class="column center-in-desktop gap-4 mx-4 mt-4">
							<MyQrCodeCard {code} {color} />

							<QrActionButtons
								{isMobile}
								onShare={() => shareCode(code)}
								onSave={() => saveCode(code, color)}
								onOpenColorPicker={openColorPicker}
							/>

							<span class="mx-2 mb-2 text-center quiet" style="font-size: 13px"
								>{m.shareCodeWarning()}</span
							>

							<div class="column gap-1">
								<List
									nested
									strongIos
									inset={isWideScreen.value || theme === 'ios'}
								>
									<ListInput
										floatingLabel
										label={m.enterYourContactsCode()}
										type="text"
										data-testid="add-contact-code-input"
										onInput={async (e: Event) => {
											const target = e.target as HTMLInputElement;
											if (target.value) {
												await receiveCode(target.value);
												target.value = '';
											}
										}}
									/>
								</List>
							</div>
						</div>
					</div>
				{/await}
			{/await}
		{:else}
			<QrCodeScanner
				bind:this={scannerRef}
				onSelectImage={receiveCode}
				onRequestPickFile={onScannerRequestPickFile}
			/>
		{/if}
	</Page>
{/if}
