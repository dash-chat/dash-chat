<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend } from '@mdi/js';

	interface Props {
		disabled?: boolean;
		onSend: () => Promise<boolean>;
		testid?: string;
	}

	let {
		disabled = false,
		onSend,
		testid = 'message-input-send',
	}: Props = $props();

	let loading = $state(false);

	async function handleClick() {
		if (loading) return;
		loading = true;
		try {
			await onSend();
		} finally {
			loading = false;
		}
	}
</script>

<button
	type="button"
	class="send-button flex h-[42px] w-[42px] shrink-0 items-center justify-center p-0"
	data-testid={testid}
	class:enabled={!disabled}
	onclick={handleClick}
	disabled={disabled || loading}
	aria-label={m.send()}
>
	{#if loading}
		<Preloader class="h-[22px] w-[22px] !text-white" />
	{:else}
		<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}></wa-icon>
	{/if}
</button>

<style>
	.send-button {
		border: none;
		border-radius: 50%;
		cursor: pointer;
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.4;
		transition:
			background-color 0.2s ease,
			opacity 0.2s ease,
			transform 0.1s ease;
	}

	.send-button:disabled {
		cursor: default;
	}

	.send-button.enabled {
		background: var(--color-brand-primary);
		color: white;
		opacity: 1;
	}

	.send-button.enabled:hover {
		filter: brightness(1.1);
	}

	.send-button.enabled:active {
		transform: scale(0.95);
	}

	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
		margin-inline-start: 2px; /* Optical centering for send arrow */
	}
</style>
