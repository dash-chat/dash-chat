<script lang="ts">
	import { onMount } from 'svelte';
	import { Toast } from 'konsta/svelte';
	import { TOAST_TTL_MS, type ToastEvent } from '$lib/utils/toasts';

	let toastOpen = $state(false);
	let toastMessage = $state('');
	let toastVariant = $state<'default' | 'error'>('default');
	let toastTimeout: ReturnType<typeof setTimeout> | undefined;

	function handleToast(event: CustomEvent<ToastEvent>) {
		clearTimeout(toastTimeout);
		toastMessage = event.detail.message;
		toastVariant = event.detail.variant ?? 'default';
		toastOpen = true;
		toastTimeout = setTimeout(() => {
			toastOpen = false;
		}, TOAST_TTL_MS);
	}

	onMount(() => {
		window.addEventListener('app:toast', handleToast as EventListener);
		return () => {
			window.removeEventListener('app:toast', handleToast as EventListener);
			clearTimeout(toastTimeout);
		};
	});
</script>

<Toast
	style="pointer-events: none"
	position="center"
	class={toastVariant === 'error' ? 'k-color-brand-red' : ''}
	opened={toastOpen}
>
	{toastMessage}
</Toast>
