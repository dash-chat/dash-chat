<script lang="ts">
	import CreateProfile from '$lib/components/profiles/CreateProfile.svelte';
	import Eula from './Eula.svelte';
	import Welcome from './Welcome.svelte';

	// A deleted account's localStorage.clear() can be lost on Android, whose
	// WebView commits storage on a timer the exit outruns; a boot without a
	// profile is the only way here, so clearing again makes the wipe stick.
	localStorage.clear();

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
