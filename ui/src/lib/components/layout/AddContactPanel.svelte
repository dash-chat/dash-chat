<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import {
		fullName,
		pendingChatKey,
		type ContactsStore,
		type SettingsStore,
	} from 'dash-chat-stores';
	import type { AddContactError } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';

	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';
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
	import { goto, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { showToast } from '$lib/utils/toasts';
	import { saveQrCode, shareQrCode } from '$lib/utils/save-qr-code';
	import {
		toDeepLink,
		extractCodeFromDeepLink,
	} from '$lib/deep-links/add-contact';
	import { defaultQrColor } from '$lib/utils/qrcode';
	import SelectColor from './SelectColor.svelte';
	import QrCodeCard from '$lib/components/QrCodeCard.svelte';
	import QrActionButtons from '$lib/components/contacts/QrActionButtons.svelte';
	import QrCodeScanner from '$lib/components/contacts/QrCodeScanner.svelte';
	import QrCodeUploader from '$lib/components/contacts/QrCodeUploader.svelte';

	type TabName = 'code' | 'scan';

	let { showBack = true }: { showBack?: boolean } = $props();

	const theme = $derived(useTheme());

	const contactsStore: ContactsStore = getContext('contacts-store');
	const settingsStore: SettingsStore = getContext('settings-store');

	let myCode = contactsStore.client.createContactCode();
	let myName = getMyName();

	let tab = $state<TabName>('code');
	let scannerRef: QrCodeScanner | null = $state(null);
	let uploaderRef: QrCodeUploader | null = $state(null);

	$effect(() => {
		const code = page.url.searchParams.get('code');
		if (code) void handleCodeFromQueryParam(code);
	});

	async function handleCodeFromQueryParam(code: string) {
		const url = new URL(page.url);
		url.searchParams.delete('code');
		replaceState(url, page.state);

		await receiveCode(code);
	}

	async function receiveDeepLink(input: string) {
		await receiveCode(extractCodeFromDeepLink(input.trim()));
	}

	async function receiveCode(code: string) {
		try {
			const myCodeString = await myCode;

			if (code === myCodeString) {
				showToast(m.cantAddYourselfAsContact(), 'error');
				return;
			}

			const devicePubkey = await contactsStore.client.addContact(code);
			showToast(m.contactRequestSent());

			const knownAgent =
				await contactsStore.client.agentForDevice(devicePubkey);
			goto(`/direct-chats/${knownAgent ?? pendingChatKey(devicePubkey)}`);
		} catch (e) {
			console.error(e);
			const error = e as AddContactError;
			switch (error.kind) {
				case 'ProfileNotCreated':
					showToast(m.errorAddContactProfileRequired(), 'error');
					break;
				case 'InvalidContactCode':
					showToast(m.errorAddContactInvalidCode(), 'error');
					break;
				case 'AddDeviceNotSupported':
					showToast(m.errorAddContactDeviceLinkingNotSupported(), 'error');
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
	let colorForPicker = $state(defaultQrColor());

	async function getMyName(): Promise<string> {
		const profile = await contactsStore.myProfile();
		return profile ? fullName(profile) : '';
	}

	async function shareCode(code: string) {
		try {
			const name = await getMyName();
			const color = (await settingsStore.qrColor()) ?? defaultQrColor();
			await shareQrCode(code, color, name);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	async function openColorPicker() {
		colorForPicker = (await settingsStore.qrColor()) ?? defaultQrColor();
		colorPickerOpen = true;
	}

	async function saveCode(code: string, color: string) {
		try {
			const name = await getMyName();
			await saveQrCode(code, color ?? defaultQrColor(), name);
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	async function switchTab(nextTab: TabName) {
		if (nextTab === tab) return;

		if (tab === 'scan' && nextTab !== 'scan' && scannerRef) {
			await scannerRef.cancelScanner();
		}

		tab = nextTab;
	}
</script>

{#if colorPickerOpen}
	{#await myCode then code}
		<SelectColor
			code={toDeepLink(code)}
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
									active={tab === 'scan'}
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
			{#await Promise.all([myCode, myName])}
				<div
					class="column"
					style="height: 100%; align-items: center; justify-content: center"
				>
					<Preloader />
				</div>
			{:then [code, name]}
				{#await $qrColor then savedColor}
					{@const color = savedColor ?? defaultQrColor()}
					<div class="column" style="flex:1">
						<div class="column center-in-desktop gap-4 mx-4 mt-4">
							<QrCodeCard
								value={toDeepLink(code)}
								label={name}
								{color}
								copyButtonTestId="add-contact-copy-btn"
								copiedMessage={m.copiedCodeToClipboard()}
							/>

							<QrActionButtons
								{isMobile}
								onShare={() => shareCode(toDeepLink(code))}
								onSave={() => saveCode(toDeepLink(code), color)}
								onUpload={() => uploaderRef?.trigger()}
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
										outline
										data-testid="add-contact-code-input"
										onInput={async (e: Event) => {
											const target = e.target as HTMLInputElement;
											if (target.value) {
												await receiveDeepLink(target.value);
												target.value = '';
											}
										}}
									/>
								</List>
							</div>
						</div>
					</div>
					<QrCodeUploader
						bind:this={uploaderRef}
						onSelectImage={receiveDeepLink}
					/>
				{/await}
			{/await}
		{:else if tab === 'scan'}
			<QrCodeScanner bind:this={scannerRef} onSelectImage={receiveDeepLink} />
		{/if}
	</Page>
{/if}
