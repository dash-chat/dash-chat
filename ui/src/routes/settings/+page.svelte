<script lang="ts">
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import EmptyState from '$lib/components/layout/EmptyState.svelte';
	import SettingsPanel from '$lib/components/layout/SettingsPanel.svelte';
	import { Page } from 'konsta/svelte';
</script>

{#if isWideScreen.value}
	<EmptyState />
{:else}
	<Page>
		<SettingsPanel />
	</Page>
{/if}
