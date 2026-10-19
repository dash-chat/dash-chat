<script lang="ts">
	import { type SettingsStore } from 'dash-chat-stores';
	import { getContext, type Snippet } from 'svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import WarningBadge from './WarningBadge.svelte';

	let { children }: { children: Snippet } = $props();

	const settingsStore: SettingsStore = getContext('settings-store');
	const localHubBlocked = useReactiveValue(settingsStore.localHubBlocked);
</script>

<WarningBadge show={$localHubBlocked === true}>
	{@render children()}
</WarningBadge>
