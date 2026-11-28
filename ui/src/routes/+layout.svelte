<script lang="ts">
	import '@awesome.me/webawesome/dist/styles/webawesome.css';
	import '@awesome.me/webawesome/dist/styles/themes/default.css';

	import '../app.css';
	import 'tauri-plugin-virtual-keyboard';
	import { setContext } from 'svelte';

	import {
		invokeAfterSetup,
		ChatsClient,
		ChatsStore,
		LogsStore,
		TauriLogsClient,
		type Payload,
		ContactsClient,
		ContactsStore,
		DevicesClient,
		DevicesStore,
		SettingsClient,
		SettingsStore,
		MailboxTrackerStore,
		type IMailboxTrackerStore,
		OnlinePeersStore,
		type IOnlinePeersStore,
		TombstoneClient,
		TombstoneStore,
		MessageAckClient,
		MessageAckStore,
	} from 'dash-chat-stores';
	import { App, KonstaProvider, Preloader } from 'konsta/svelte';

	import OnboardingWrapper from '$lib/components/onboarding/OnboardingWrapper.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';
	import CrashReportDialog from '$lib/components/CrashReportDialog.svelte';
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import MobileLayout from '$lib/components/layout/MobileLayout.svelte';
	import { addContactPending } from '$lib/stores/add-contact-pending.svelte';
	import { modalHost } from '$lib/stores/modal-host.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactivePromise, useSignal } from '$lib/stores/use-signal';
	import { applyDarkMode } from '$lib/utils/theme';
	import { isIos, isMobile, isTauriEnv } from '$lib/utils/environment';
	import {
		forwardConsoleToTauriLog,
		reportUncaughtErrors,
	} from '$lib/utils/logs';
	import { handleUrls } from '$lib/utils/deep-links';
	import { followPendingNavigations } from '$lib/utils/pending-navigations';

	import { m } from '$lib/paraglide/messages.js';
	import { getLocale, type Locale, setLocale } from '$lib/paraglide/runtime';
	import { goto } from '$app/navigation';
	import { useKeepAlive } from '$lib/stores/keep-alive-scope.svelte';
	import { previewFeatures } from '$lib/stores/preview-features.svelte';
	import { registerSetLocale } from '$lib/utils/locale';
	import { installPreviewTauri } from '$lib/utils/preview-tauri';

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
		// runtime anyway.
		registerTestUtils(
			goto,
			setLocale as (locale: string) => void,
			m,
			() => previewFeatures.enable(),
			url => handleUrls([url], contactsStore),
		),
	);

	// Forward console.log/info/warn/error from the WebView to the tauri logs
	forwardConsoleToTauriLog();
	reportUncaughtErrors();

	let { children } = $props();

	if (!isTauriEnv()) installPreviewTauri();

	const logsClient = new TauriLogsClient<Payload>();
	const logsStore = new LogsStore<Payload>(logsClient);
	const settingsStore = new SettingsStore(new SettingsClient());

	const devicesClient = new DevicesClient();
	const devicesStore = new DevicesStore(logsStore, devicesClient);

	const contactsClient = new ContactsClient(logsClient);
	const contactsStore = new ContactsStore(
		logsStore,
		devicesStore,
		contactsClient,
	);

	const tombstoneStore = new TombstoneStore(new TombstoneClient());
	const mailboxTrackerStore = new MailboxTrackerStore();
	const messageAckStore = new MessageAckStore(
		new MessageAckClient(),
		mailboxTrackerStore,
	);

	const chatsClient = new ChatsClient();
	const chatsStore = new ChatsStore(
		logsStore,
		contactsStore,
		tombstoneStore,
		messageAckStore,
		chatsClient,
	);
	const onlinePeersStore = new OnlinePeersStore(contactsStore, chatsStore);

	invokeAfterSetup('log_webview_info', {
		userAgent: navigator.userAgent,
	}).catch(() => {});

	setContext('settings-store', settingsStore);
	setContext('devices-store', devicesStore);
	setContext('contacts-store', contactsStore);
	setContext('chats-store', chatsStore);
	setContext('mailbox-tracker-store', mailboxTrackerStore);
	setContext('online-peers-store', onlinePeersStore);

	// Keep the chats summaries signal warm so it's always fully loaded
	// when navigating back home from any page
	useKeepAlive(chatsStore.allChatsSummaries);
	useKeepAlive(mailboxTrackerStore.connectionStatusForAllMailboxes);

	// Nothing routed renders until the device id has resolved: pages read it
	// synchronously (useDeviceId), and a notification-tap navigation can mount
	// them before the lazy myDeviceId reactive has ever been started.
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	let theme: 'ios' | 'material' = $state(isIos ? 'ios' : 'material');

	const applied = useSignal(settingsStore.colorScheme);

	let darkOverride: boolean | null = $state(null);
	const effectiveDark = $derived(darkOverride ?? $applied === 'dark');
	$effect(() => {
		applyDarkMode(effectiveDark);
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

	$effect(() => {
		if (!isTauriEnv()) return;
		return followPendingNavigations(contactsStore);
	});
</script>

<!-- On a phone a long press on any link or image would otherwise pick it up
     and float its title and URL under the finger. Drags the app asks for with
     draggable="true" still go through. -->
<svelte:document
	ondragstart={e => {
		const optedIn =
			e.target instanceof Element && e.target.closest('[draggable="true"]');
		if (isMobile && !optedIn) e.preventDefault();
	}}
/>

<KonstaProvider {theme} dark={effectiveDark}>
	<App safeAreas {theme} class="k-{theme}" dark={effectiveDark}>
		{#await $myDeviceId}
			<div
				class="column"
				style="height: 100vh; width: 100vw; align-items: center; justify-content: center"
			>
				<Preloader></Preloader>
			</div>
		{:then}
			<OnboardingWrapper>
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
			</OnboardingWrapper>
		{/await}
		{#if addContactPending.value}
			<div
				class="fixed inset-0 z-40 flex items-center justify-center"
				style="background-color: var(--background-color)"
				data-testid="add-contact-pending-overlay"
			>
				<Preloader />
			</div>
		{/if}
		<ToastManager />
		{#if import.meta.env.VITE_SENTRY_ENABLED}
			<CrashReportDialog />
		{/if}
		<div class="contents" bind:this={modalHost.element}></div>
	</App>
</KonstaProvider>
