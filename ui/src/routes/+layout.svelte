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
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useSignal } from '$lib/stores/use-signal';
	import { applyDarkMode } from '$lib/utils/theme';
	import { showToast } from '$lib/utils/toasts';

	import { m } from '$lib/paraglide/messages.js';
	import { setLocale } from '$lib/paraglide/runtime';
	window.__setLocale = setLocale;

	if (import.meta.env.DEV) {
		import('../../tests/setup-utils').then(({ registerTestUtils }) => registerTestUtils());
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

	$effect(() => {
		applyDarkMode($isDark).catch((e) => {
			showToast(m.errorApplyStyle(), 'error');
		});
	});

	let theme: 'ios' | 'material' = $state('material');

	$effect(() => {
		const handler = (event: CustomEvent) => {
			theme = event.detail.theme;
		};
		window.addEventListener('theme-change', handler as EventListener);
		return () => window.removeEventListener('theme-change', handler as EventListener);
	});

</script>

<KonstaProvider {theme}>
	<App safeAreas {theme} class={`k-${theme}`} dark={!!$isDark}>
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
