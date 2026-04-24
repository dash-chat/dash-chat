<script lang="ts">
	import '@awesome.me/webawesome/dist/styles/webawesome.css';
	import '@awesome.me/webawesome/dist/styles/themes/default.css';

	import '../app.css';
	import { setContext } from 'svelte';

	import {
		ChatsClient,
		ChatsStore,
		LogsStore,
		TauriLogsClient,
		LocalStorageLogsClient,
		type Payload,
		ContactsClient,
		ContactsStore,
		DevicesClient,
		DevicesStore,
		SettingsClient,
		SettingsStore,
		MockContactsClient,
		MockDevicesClient,
		MockChatsClient,
		MockDirectChatClient,
		MockGroupChatClient,
		MockSettingsClient,
		seedDemoData,
		DEMO_IDS,
	} from 'dash-chat-stores';
	import { App, KonstaProvider } from 'konsta/svelte';

	import SplashscreenPrompt from '$lib/components/splashscreen/SplashscreenPrompt.svelte';
	import PreviewToolbar from '$lib/components/preview/PreviewToolbar.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';
	import TestBanner from '$lib/components/layout/TestBanner.svelte';
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useSignal } from '$lib/stores/use-signal';
	import { applyDarkMode } from '$lib/utils/theme';
	import { showToast } from '$lib/utils/toasts';
	import { isIos, isMac, isMobile, isTauriEnv } from '$lib/utils/environment';

	import { m } from '$lib/paraglide/messages.js';
	import { setLocale } from '$lib/paraglide/runtime';
	import { goto } from '$app/navigation';
	window.__setLocale = setLocale;

	import('../../tests/setup-utils').then(({ registerTestUtils }) =>
		registerTestUtils(goto),
	);

	let { children } = $props();

	const isPreview = !isTauriEnv();
	const showToolbar = (isPreview || import.meta.env.DEV) && !isMobile;

	// --- Store initialization ---
	let settingsStore: SettingsStore;
	let logsStore: LogsStore<Payload>;
	let devicesStore: DevicesStore;
	let contactsStore: ContactsStore;
	let chatsStore: ChatsStore;

	if (isPreview) {
		const mockLogsClient = new LocalStorageLogsClient(DEMO_IDS.MY_DEVICE_ID);
		seedDemoData(mockLogsClient);

		logsStore = new LogsStore<Payload>(mockLogsClient);
		settingsStore = new SettingsStore(new MockSettingsClient());

		const mockDevicesClient = new MockDevicesClient(
			DEMO_IDS.DEVICE_GROUP_TOPIC,
		);
		devicesStore = new DevicesStore(logsStore, mockDevicesClient);

		const mockContactsClient = new MockContactsClient(
			mockLogsClient,
			DEMO_IDS.MY_AGENT_ID,
			DEMO_IDS.MY_DEVICE_ID,
			DEMO_IDS.DEVICE_GROUP_TOPIC,
			[DEMO_IDS.INBOX_TOPIC],
		);
		contactsStore = new ContactsStore(
			logsStore,
			devicesStore,
			mockContactsClient,
		);

		const mockChatsClient = new MockChatsClient();
		chatsStore = new ChatsStore(
			logsStore,
			contactsStore,
			mockChatsClient,
			() => new MockDirectChatClient(mockLogsClient, DEMO_IDS.MY_AGENT_ID),
			() => new MockGroupChatClient(),
		);
	} else {
		const logsClient = new TauriLogsClient<Payload>();
		logsStore = new LogsStore<Payload>(logsClient);
		settingsStore = new SettingsStore(new SettingsClient());

		const devicesClient = new DevicesClient();
		devicesStore = new DevicesStore(logsStore, devicesClient);

		const contactsClient = new ContactsClient(logsClient);
		contactsStore = new ContactsStore(logsStore, devicesStore, contactsClient);

		const chatsClient = new ChatsClient();
		chatsStore = new ChatsStore(logsStore, contactsStore, chatsClient);
	}

	setContext('settings-store', settingsStore);
	setContext('devices-store', devicesStore);
	setContext('contacts-store', contactsStore);
	setContext('chats-store', chatsStore);

	const isDark = useSignal(settingsStore.isDark);

	let theme: 'ios' | 'material' = $state(isIos || isMac ? 'ios' : 'material');

	let darkOverride: boolean | null = $state(null);
	const effectiveDark = $derived(darkOverride ?? !!$isDark);
	$effect(() => {
		applyDarkMode(effectiveDark).catch(e => {
			showToast(m.errorApplyStyle(), 'error');
		});
	});

	$effect(() => {
		const handler = (event: CustomEvent) => {
			theme = event.detail.theme;
		};
		window.addEventListener('theme-change', handler as EventListener);
		return () =>
			window.removeEventListener('theme-change', handler as EventListener);
	});

	$effect(() => {
		const handler = (event: CustomEvent) => {
			darkOverride = event.detail;
		};
		window.addEventListener('set-dark-mode', handler as EventListener);
		return () =>
			window.removeEventListener('set-dark-mode', handler as EventListener);
	});
</script>

{#if showToolbar}
	<PreviewToolbar />
{/if}

<KonstaProvider {theme} dark={effectiveDark}>
	<App safeAreas {theme} class="k-{theme}" dark={effectiveDark}>
		<TestBanner text="Early access test" />
		<SplashscreenPrompt>
			{#if isWideScreen.value}
				<DesktopLayout>
					{@render children()}
				</DesktopLayout>
			{:else}
				{@render children()}
			{/if}
		</SplashscreenPrompt>
		<ToastManager />
	</App>
</KonstaProvider>
