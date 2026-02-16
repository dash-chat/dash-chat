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
	} from 'dash-chat-stores';
	import { App, KonstaProvider } from 'konsta/svelte';

	import SplashscreenPrompt from '$lib/components/splashscreen/SplashscreenPrompt.svelte';
	import ToastManager from '$lib/components/toast/ToastManager.svelte';

	import { setLocale } from '$lib/paraglide/runtime';
	setLocale('en');
	window.__setLocale = setLocale;

	let { children } = $props();

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
			{@render children()}
		</SplashscreenPrompt>
		<ToastManager />
	</App>
</KonstaProvider>
