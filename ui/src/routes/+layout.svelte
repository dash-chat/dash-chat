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
		PreferencesClient,
		PreferencesStore,
	} from 'dash-chat-stores';
	import { App, KonstaProvider } from 'konsta/svelte';

	import SplashscreenPrompt from '$lib/components/splashscreen/SplashscreenPrompt.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';
	import DesktopLayout from '$lib/components/layout/DesktopLayout.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	import { setLocale, getLocale } from '$lib/paraglide/runtime';
	window.__setLocale = setLocale;

	if (import.meta.env.DEV) {
		import('../../tests/setup-utils').then(({ registerTestUtils }) => registerTestUtils());
	}

	let { children } = $props();

	const preferencesClient = new PreferencesClient();
	const preferencesStore = new PreferencesStore(preferencesClient, getLocale(), setLocale);
	setContext('preferences-store', preferencesStore)

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

	let theme: 'ios' | 'material' = $state('material');

	window.addEventListener('theme-change', (event: CustomEvent) => {
		theme = event.detail.theme;
	});
</script>

<KonstaProvider {theme}>
	<App safeAreas {theme} class={`k-${theme}`}>
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
