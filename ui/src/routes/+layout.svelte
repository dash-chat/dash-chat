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
		type Payload,
		ContactsClient,
		ContactsStore,
		DevicesClient,
		DevicesStore,
		SettingsClient,
		SettingsStore,
	} from 'dash-chat-stores';
	import { App, KonstaProvider } from 'konsta/svelte';

	import SplashscreenPrompt from '$lib/components/splashscreen/SplashscreenPrompt.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';
	import UpdaterDialog from '$lib/components/UpdaterDialog.svelte';
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useSignal } from '$lib/stores/use-signal';
	import { applyDarkMode } from '$lib/utils/theme';
	import { showToast } from '$lib/utils/toasts';
	import { isIos } from '$lib/utils/environment';

	import { m } from '$lib/paraglide/messages.js';
	import { setLocale } from '$lib/paraglide/runtime';
	import { goto } from '$app/navigation';
	window.__setLocale = setLocale;

	// Register test utils in dev mode and E2E builds (VITE_E2E=1)
	if (import.meta.env.DEV || import.meta.env.VITE_E2E) {
		import('../../tests/setup-utils').then(({ registerTestUtils }) => registerTestUtils(goto));
	}

	let { children } = $props();

	const settingsStore = new SettingsStore(new SettingsClient());
	setContext('settings-store', settingsStore);

	const logsClient = new TauriLogsClient<Payload>();
	const logsStore = new LogsStore<Payload>(logsClient);

	const devicesClient = new DevicesClient();
	const devicesStore = new DevicesStore(logsStore, devicesClient);
	setContext('devices-store', devicesStore);

	const contactsClient = new ContactsClient(logsClient);
	const contactsStore = new ContactsStore(
		logsStore,
		devicesStore,
		contactsClient,
	);
	setContext('contacts-store', contactsStore);

	const chatsClient = new ChatsClient();
	const chatsStore = new ChatsStore(logsStore, contactsStore, chatsClient);
	setContext('chats-store', chatsStore);

	const isDark = useSignal(settingsStore.isDark);

		let theme: 'ios' | 'material' = $state(isIos ? 'ios' : 'material');

	let darkOverride: boolean | null = $state(null);
	const effectiveDark = $derived(darkOverride ?? !!$isDark);

	$effect(() => {
		applyDarkMode(effectiveDark).catch((e) => {
			showToast(m.errorApplyStyle(), 'error');
		});
	});

	$effect(() => {
		const handler = (event: CustomEvent) => {
			theme = event.detail.theme;
		};
		window.addEventListener('theme-change', handler as EventListener);
		return () => window.removeEventListener('theme-change', handler as EventListener);
	});

	$effect(() => {
		const handler = (event: CustomEvent) => {
			darkOverride = event.detail;
		};
		window.addEventListener('set-dark-mode', handler as EventListener);
		return () => window.removeEventListener('set-dark-mode', handler as EventListener);
	});

</script>

<KonstaProvider {theme} dark={effectiveDark}>
	<App safeAreas {theme} class={`k-${theme}`} dark={effectiveDark}>
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
		<UpdaterDialog />
	</App>
</KonstaProvider>
