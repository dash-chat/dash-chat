<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCheck, mdiSend } from '@mdi/js';

	interface Props {
		onSend: () => Promise<boolean>;
		/** Show a checkmark (save edit) instead of the send arrow. */
		editing?: boolean;
		testid?: string;
	}

	let {
		onSend,
		editing = false,
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
	onclick={handleClick}
	disabled={loading}
	aria-label={editing ? m.save() : m.send()}
>
	{#if loading}
		<Preloader class="h-[22px] w-[22px] !text-white" />
	{:else if editing}
		<wa-icon
			class="no-offset"
			style="font-size: 24px"
			src={wrapPathInSvg(mdiCheck)}
		></wa-icon>
	{:else}
		<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}></wa-icon>
	{/if}
</button>

<style>
	.send-button {
		border: none;
		border-radius: 50%;
		cursor: pointer;
		background: var(--color-brand-primary);
		color: white;
		transition:
			filter 0.2s ease,
			transform 0.1s ease;
	}

	.send-button:disabled {
		cursor: default;
	}

	.send-button:hover {
		filter: brightness(1.1);
	}

	.send-button:active {
		transform: scale(0.95);
	}

	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
		margin-inline-start: 2px; /* Optical centering for send arrow */
	}

	.send-button :global(wa-icon.no-offset) {
		margin-inline-start: 0;
	}
</style>
