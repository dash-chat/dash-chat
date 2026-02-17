<script lang="ts">
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import EmptyState from '$lib/components/layout/EmptyState.svelte';
	import NewMessagePanel from '$lib/components/layout/NewMessagePanel.svelte';
	import { Page } from 'konsta/svelte';
</script>

{#if isWideScreen.value}
	<EmptyState />
{:else}
	<Page>
		<NewMessagePanel />
	</Page>
{/if}
