<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { Navbar, NavbarBackLink, Page } from 'konsta/svelte';
</script>

<Page data-testid="developer-page">
	<Navbar title={m.developer()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="developer-back"
				/>
			{/if}
		{/snippet}
	</Navbar>
</Page>
