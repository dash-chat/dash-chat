<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { getContext } from 'svelte';
	import {
		fullName,
		type ContactsStore,
		type SettingsStore,
	} from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Preloader,
		Button,
		useTheme,
		ToolbarPane,
		TabbarLink,
		Tabbar,
	} from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';
	import { mdiContentCopy } from '@mdi/js';
	import { copyLinkToClipboard } from '$lib/utils/clipboard';
	import BorderedBox from '$lib/components/BorderedBox.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import { saveQrCode, shareQrCode } from '$lib/utils/save-qr-code';
	import {
		toDeepLink,
		addContactFromDeepLink,
	} from '$lib/deep-links/add-contact';
	import { defaultQrColor } from '$lib/utils/qrcode';
	import SelectColor from './SelectColor.svelte';
	import QrCodeCard from '$lib/components/QrCodeCard.svelte';
	import QrActionButtons from '$lib/components/contacts/QrActionButtons.svelte';
	import QrLinkSheet from '$lib/components/contacts/QrLinkSheet.svelte';
	import QrCodeScanner from '$lib/components/contacts/QrCodeScanner.svelte';
	import QrCodeUploader from '$lib/components/contacts/QrCodeUploader.svelte';

	type TabName = 'code' | 'scan';

	let { showBack = true }: { showBack?: boolean } = $props();

	const theme = $derived(useTheme());

	const contactsStore: ContactsStore = getContext('contacts-store');
	const settingsStore: SettingsStore = getContext('settings-store');

	let myCode = contactsStore.client.createContactCode();
	let myName = getMyName();
	let myDeepLink = myCode.then(code => {
		const link = toDeepLink(code);
		if (link === null) {
			console.error('toDeepLink returned null for code', code);
			showToast(m.errorUnexpected(), 'error');
		}
		return link;
	});

	let tab = $state<TabName>('code');
	let scannerRef: QrCodeScanner | null = $state(null);
	let uploaderRef: QrCodeUploader | null = $state(null);

	function receiveDeepLink(link: string) {
		return addContactFromDeepLink(contactsStore, link);
	}

	const qrColor = useReactivePromise(settingsStore.qrColor);
	let colorPickerOpen = $state(false);
	let colorForPicker = $state(defaultQrColor());
	let linkSheetOpen = $state(false);

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
	{#await Promise.all([myDeepLink, myName])}
		<Preloader />
	{:then [deepLink, name]}
		{#if deepLink !== null}
			<SelectColor
				qrCodeValue={deepLink}
				qrCodeLabel={name}
				qrColor={colorForPicker}
				onClose={() => (colorPickerOpen = false)}
			/>
		{/if}
	{:catch}
		<!-- -->
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
								data-testid="add-contact-link-tab"
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
									data-testid="add-contact-link-tab"
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
			{#await Promise.all([myDeepLink, myName])}
				<div
					class="column"
					style="height: 100%; align-items: center; justify-content: center"
				>
					<Preloader />
				</div>
			{:then [deepLink, name]}
				{#if deepLink !== null}
					{#await $qrColor then savedColor}
						{@const color = savedColor ?? defaultQrColor()}
						<div class="column" style="flex:1">
							<div class="column center-in-desktop gap-4 mx-4 mt-4">
								<QrCodeCard
									value={deepLink}
									label={name}
									{color}
									copyButtonTestId="add-contact-copy-btn"
								/>

								<QrActionButtons
									{isMobile}
									onLink={() => {
										linkSheetOpen = true;
									}}
									onShare={() => shareCode(deepLink)}
									onSave={() => saveCode(deepLink, color)}
									onUpload={() => uploaderRef?.trigger()}
									onOpenColorPicker={openColorPicker}
								/>

								<QrLinkSheet
									opened={linkSheetOpen}
									link={deepLink}
									onClose={() => (linkSheetOpen = false)}
								/>

								{#if !isMobile}
									<BorderedBox
										class="row w-full items-center gap-3"
										data-testid="add-contact-copy-link-box"
									>
										<IconButton
											icon={mdiContentCopy}
											label={m.copy()}
											testid="add-contact-copy-link-btn"
											onClick={() => void copyLinkToClipboard(deepLink)}
											class="shrink-0"
										/>
										<span class="break-all text-start text-sm">{deepLink}</span>
									</BorderedBox>
								{/if}

								<span
									class="mx-6 mb-2 text-center quiet"
									style="font-size: 13px">{m.shareCodeWarning()}</span
								>
							</div>
						</div>
						<QrCodeUploader
							bind:this={uploaderRef}
							onSelectImage={receiveDeepLink}
						/>
					{/await}
				{/if}
			{:catch}
				<!-- -->
			{/await}
		{:else if tab === 'scan'}
			<QrCodeScanner bind:this={scannerRef} onSelectImage={receiveDeepLink} />
		{/if}
	</Page>
{/if}
