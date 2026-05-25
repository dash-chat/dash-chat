<script lang="ts">
	import '@awesome.me/webawesome/dist/styles/webawesome.css';
	import '@awesome.me/webawesome/dist/styles/themes/default.css';

	import '../app.css';
	import { setContext } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';

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
		MailboxTrackerStore,
		type IMailboxTrackerStore,
		MockContactsClient,
		MockDevicesClient,
		MockChatsClient,
		MockDirectChatClient,
		MockGroupChatClient,
		MockMailboxTrackerStore,
		MockSettingsClient,
		seedDemoData,
		DEMO_IDS,
	} from 'dash-chat-stores';
	import { App, KonstaProvider } from 'konsta/svelte';

	import SplashscreenPrompt from '$lib/components/splashscreen/SplashscreenPrompt.svelte';
	import PreviewToolbar from '$lib/components/preview/PreviewToolbar.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import MobileLayout from '$lib/components/layout/MobileLayout.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise, useSignal } from '$lib/stores/use-signal';
	import { applyDarkMode } from '$lib/utils/theme';
	import { showToast } from '$lib/utils/toasts';
	import { isIos, isMac, isMobile, isTauriEnv } from '$lib/utils/environment';
	import { forwardConsoleToTauriLog } from '$lib/utils/logs';

	import { m } from '$lib/paraglide/messages.js';
	import { getLocale, type Locale, setLocale } from '$lib/paraglide/runtime';
	import { goto } from '$app/navigation';
	import { useKeepAlive } from '$lib/stores/keep-alive-scope.svelte';
	import { registerSetLocale } from '$lib/utils/locale';

	// TODO: once the language-selector setting lands, make that setting the
	// source of truth for this state (read it via `useSignal(settingsStore.locale)`
	// or similar) and have the selector's onChange call into paraglide's
	// `setLocale`. The `{#key}` mechanism stays the same.
	let currentLocale = $state<Locale>(getLocale());
	registerSetLocale(locale => {
		currentLocale = locale;
	});

	import('../../tests/setup-utils').then(({ registerTestUtils }) =>
		// Paraglide types setLocale with a string-literal union; we widen to
		// plain `string` at the test boundary since invalid locales fail at
		// runtime anyway. `setLocale` here is the non-reloading variant
		// installed by `registerSetLocale` above.
		registerTestUtils(goto, setLocale as (locale: string) => void, m),
	);

	// Forward console.log/info/warn/error from the WebView to the tauri logs
	forwardConsoleToTauriLog();

	let { children } = $props();

	const isPreview = !isTauriEnv();
	const showToolbar = (isPreview || import.meta.env.DEV) && !isMobile;

	// --- Store initialization ---
	let settingsStore: SettingsStore;
	let logsStore: LogsStore<Payload>;
	let devicesStore: DevicesStore;
	let contactsStore: ContactsStore;
	let chatsStore: ChatsStore;
	let mailboxTrackerStore: IMailboxTrackerStore;

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

		mailboxTrackerStore = new MockMailboxTrackerStore();
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

		mailboxTrackerStore = new MailboxTrackerStore();

		invoke('log_webview_info', { userAgent: navigator.userAgent }).catch(
			() => {},
		);
	}

	setContext('settings-store', settingsStore);
	setContext('devices-store', devicesStore);
	setContext('contacts-store', contactsStore);
	setContext('chats-store', chatsStore);
	setContext('mailbox-tracker-store', mailboxTrackerStore);

	// Keep the chats summaries signal warm so it's always fully loaded
	// when navigating back home from any page
	useKeepAlive(chatsStore.allChatsSummaries);

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

	$effect(() => {
		document.documentElement.classList.toggle(
			'wide-layout',
			isWideScreen.value,
		);
	});
</script>

{#if showToolbar}
	<PreviewToolbar />
{/if}

<KonstaProvider {theme} dark={effectiveDark}>
	<App safeAreas {theme} class="k-{theme}" dark={effectiveDark}>
		<SplashscreenPrompt>
			{#key currentLocale}
				{#if isWideScreen.value}
					<DesktopLayout>
						{@render children()}
					</DesktopLayout>
				{:else}
					<MobileLayout>
						{@render children()}
					</MobileLayout>
				{/if}
			{/key}
		</SplashscreenPrompt>
		<ToastManager />
	</App>
</KonstaProvider>
