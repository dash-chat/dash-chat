<script lang="ts">
	import CreateProfile from '$lib/components/profiles/CreateProfile.svelte';
	import Eula from './Eula.svelte';
	import Welcome from './Welcome.svelte';

	let step = $state<'welcome' | 'eula' | 'profile'>('welcome');
</script>

{#if step === 'welcome'}
	<Welcome
		onContinue={() => (step = 'profile')}
		onTerms={() => (step = 'eula')}
	/>
{:else if step === 'eula'}
	<Eula onBack={() => (step = 'welcome')} />
{:else}
	<CreateProfile onBack={() => (step = 'welcome')} />
{/if}
