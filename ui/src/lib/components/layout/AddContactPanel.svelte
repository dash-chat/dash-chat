<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/qr-code/qr-code.js';
	import '@awesome.me/webawesome/dist/components/copy-button/copy-button.js';
	import { getContext } from 'svelte';
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import {
		decodeContactCode,
		encodeContactCode,
		fullName,
		toPromise,
		type ContactsStore,
		type SettingsStore,
	} from 'dash-chat-stores';
	import type { AddContactError } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiContentCopy,
		mdiLinkVariant,
		mdiShareVariant,
		mdiTrayArrowDown,
		mdiPalette,
		mdiImageSearchOutline,
	} from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';

	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';
	import { scanQrcode, scanQrFromImage } from '$lib/utils/qrcode';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		ListInput,
		List,
		Card,
		Preloader,
		Button,
		useTheme,
		ToolbarPane,
		TabbarLink,
		Tabbar,
		Dialog,
		DialogButton,
	} from 'konsta/svelte';
	import { goto } from '$app/navigation';
	import { showToast } from '$lib/utils/toasts';
	import { cancel } from '@tauri-apps/plugin-barcode-scanner';
	import { saveQrCode, shareQrCode } from '$lib/utils/save-qr-code';
	import SelectColor from './SelectColor.svelte';

	let { showBack = true }: { showBack?: boolean } = $props();

	const theme = $derived(useTheme());

	const contactsStore: ContactsStore = getContext('contacts-store');
	const settingsStore: SettingsStore = getContext('settings-store');

	let myCode = $state(
		contactsStore.client.getOrCreateContactCode().then(encodeContactCode),
	);
	let resetDialogOpen = $state(false);

	let tab = $state<'code' | 'scan'>('code');

	async function resetCode() {
		try {
			const code = await contactsStore.client.resetContactCode();
			myCode = Promise.resolve(encodeContactCode(code));
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

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
				case 'CreateContactCode':
				case 'CreateDirectChat':
					showToast(m.errorAddContact(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}

	async function scan() {
		if (tab === 'scan') return;
		tab = 'scan';
		try {
			const code = await scanQrcode();
			await receiveCode(code);
		} catch (e) {
			console.error(e);
			showToast(m.errorScanningQrCode(), 'error');
		}
	}

	async function cancelScan() {
		if (tab === 'code') return;
		tab = 'code';
		await cancel();
	}

	const qrColor = useReactivePromise(settingsStore.qrColor);
	let colorPickerOpen = $state(false);
	let colorForPicker = $state('#007aff');

	async function copyLink(code: string) {
		await writeText(code);
		showToast(m.copiedCodeToClipboard());
	}

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

	let imageFilePicker: HTMLInputElement;

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
								onClick={cancelScan}
								data-testid="add-contact-code-tab"
								>{m.code()}
							</Button>

							<Button
								class="w-24"
								small
								rounded
								tonal={tab !== 'scan'}
								onClick={scan}
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
									onclick={cancelScan}
									label={m.code()}
									data-testid="add-contact-code-tab"
								/>
								<TabbarLink
									active={tab !== 'code'}
									onclick={scan}
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
					{@const isWhite = color === '#ffffff'}
					<div class="column gap-4" style="flex: 1">
						<div class="column center-in-desktop gap-4 mx-4 mt-4">
							<Card
								class="qr-card p-2.5 pb-2"
								style="background-color: {color}"
							>
								<div class="column" style="align-items: center">
									<div
										class="column w-full p-3"
										style="align-items: center; justify-content: center; background-color: white; border-radius: 10px;"
									>
										<wa-qr-code
											value={code}
											size="180"
											fill={isWhite ? '#000000' : color}
										></wa-qr-code>
									</div>

									<div class="py-1">
										<Button
											colors={{
												touchRipple: isWhite ? 'black' : 'white',
												textIos: isWhite ? 'text-black' : 'text-white',
												textMaterial: isWhite ? 'text-black' : 'text-white',
											}}
											clearMaterial
											small
											data-testid="add-contact-copy-btn"
											onClick={async () => {
												await writeText(code);
												showToast(m.copiedCodeToClipboard());
											}}
										>
											<wa-icon src={wrapPathInSvg(mdiContentCopy)}> </wa-icon>

											{code.slice(0, 15)}...
										</Button>
									</div>
								</div>
							</Card>

							<!-- Action buttons: Link, Share, Save, Color -->
							<div class="row gap-4" style="justify-content: center;">
								<div
									class="column"
									style="display: none; align-items: center; gap: 8px;"
								>
									<Button
										tonal
										onClick={() => copyLink(code)}
										class="icon-only"
										data-testid="add-contact-link-btn"
									>
										<wa-icon
											src={wrapPathInSvg(mdiLinkVariant)}
											style="font-size: 28px"
										></wa-icon>
									</Button>
									<span class="text-sm" style="color: var(--k-text-color)"
										>{m.link()}</span
									>
								</div>

								{#if isMobile}
									<div class="column" style="align-items: center; gap: 8px;">
										<Button
											tonal
											onClick={() => shareCode(code)}
											class="icon-only"
											data-testid="add-contact-share-btn"
										>
											<wa-icon
												src={wrapPathInSvg(mdiShareVariant)}
												style="font-size: 28px"
											></wa-icon>
										</Button>
										<span class="text-sm" style="color: var(--k-text-color)"
											>{m.share()}</span
										>
									</div>
								{:else}
									<div class="column" style="align-items: center; gap: 8px;">
										<Button
											tonal
											onClick={async () => {
												try {
													const name = await getMyName();
													await saveQrCode(code, color, name);
												} catch (e) {
													console.error(e);
													showToast(m.errorUnexpected(), 'unexpected', e);
												}
											}}
											class="icon-only"
											data-testid="add-contact-save-btn"
										>
											<wa-icon
												src={wrapPathInSvg(mdiTrayArrowDown)}
												style="font-size: 28px"
											></wa-icon>
										</Button>
										<span class="text-sm" style="color: var(--k-text-color)"
											>{m.save()}</span
										>
									</div>
								{/if}

								<div class="column" style="align-items: center; gap: 8px;">
									<Button
										tonal
										onClick={async () => {
											colorForPicker = await toPromise(settingsStore.qrColor);
											colorPickerOpen = true;
										}}
										class="icon-only"
										data-testid="add-contact-color-btn"
									>
										<wa-icon
											src={wrapPathInSvg(mdiPalette)}
											style="font-size: 28px"
										></wa-icon>
									</Button>
									<span class="text-sm" style="color: var(--k-text-color)"
										>{m.color()}</span
									>
								</div>
							</div>

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
						<div class="row" style="justify-content: center">
							<Button
								style="width: auto"
								tonal
								small
								rounded
								onClick={() => (resetDialogOpen = true)}
							>
								{m.resetQrCode()}
							</Button>
						</div>
					</div>
				{/await}
			{/await}
		{:else}
			<div class="column" style="position: relative; flex: 1;">
				<div
					class="row p-4 top-2"
					style="color: white; position: absolute; width: 100%; align-items: center; justify-content: center; z-index: 1; text-align: center"
				>
					<span class="w-60">{m.scanQrCodeOfYourContact()}</span>
				</div>
				<div
					class="column"
					style="flex: 1; align-items: center; justify-content: center"
				>
					<div class="barcode-scanner--area--container">
						<div class="square surround-cover">
							<div class="barcode-scanner--area--outer surround-cover"></div>
						</div>
					</div>
				</div>
				<div
					style="position: absolute; bottom: 24px; left: 0; right: 0; display: flex; justify-content: center; z-index: 1;"
				>
					<button
						class="w-14 h-14 rounded-full bg-white text-gray-700 border-none cursor-pointer flex items-center justify-center shadow-[0_2px_8px_rgba(0,0,0,0.3)] transition-transform duration-200 hover:scale-105 active:scale-95"
						onclick={() => imageFilePicker.click()}
						aria-label={m.photo()}
						data-testid="add-contact-select-image-btn"
					>
						<wa-icon
							src={wrapPathInSvg(mdiImageSearchOutline)}
							style="font-size: 28px"
						></wa-icon>
					</button>
				</div>
			</div>
		{/if}

		<Dialog
			opened={resetDialogOpen}
			onBackdropClick={() => (resetDialogOpen = false)}
		>
			{#snippet title()}
				{m.resetQrCode()}
			{/snippet}
			<span>{m.areYouSureResetQrCode()}</span>
			{#snippet buttons()}
				<DialogButton onClick={() => (resetDialogOpen = false)}>
					{m.cancel()}
				</DialogButton>
				<DialogButton
					onClick={() => {
						resetDialogOpen = false;
						resetCode();
					}}
				>
					{m.reset()}
				</DialogButton>
			{/snippet}
		</Dialog>
	</Page>
{/if}

<style>
	:global(.qr-card) {
		align-self: center;
		width: fit-content;
		margin: 0 !important;
		transition: background-color 0.3s ease;
	}

	.square {
		width: 100%;
		position: relative;
		overflow: hidden;
		transition: 0.3s;
	}
	.square:after {
		content: '';
		top: 0;
		display: block;
		padding-bottom: 100%;
	}
	.square > div {
		position: absolute;
		top: 0;
		left: 0;
		bottom: 0;
		right: 0;
	}

	.surround-cover {
		box-shadow: 0 0 0 99999px rgba(0, 0, 0, 0.5);
	}

	.barcode-scanner--area--container {
		width: 80%;
		max-width: min(500px, 80vh);
	}
	.barcode-scanner--area--outer {
		display: flex;
		border-radius: 1em;
	}
</style>
