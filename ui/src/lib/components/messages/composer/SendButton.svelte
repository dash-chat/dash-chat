<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend } from '@mdi/js';

	interface Props {
		disabled: boolean;
		onClick: () => void;
	}

	let { disabled, onClick }: Props = $props();
</script>

<button
	type="button"
	class="send-button"
	data-testid="message-input-send"
	class:enabled={!disabled}
	onclick={onClick}
	{disabled}
	aria-label={m.send()}
>
	<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}></wa-icon>
</button>

<style>
	.send-button {
		flex-shrink: 0;
		width: 42px;
		height: 42px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
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
		background: var(--k-theme-color, #3b82f6);
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
