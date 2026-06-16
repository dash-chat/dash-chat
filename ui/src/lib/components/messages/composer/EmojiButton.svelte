<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiEmoticonHappyOutline } from '@mdi/js';

	interface Props {
		onClick: () => void;
	}

	let { onClick }: Props = $props();
</script>

<button
	type="button"
	class="icon-button emoji-btn"
	onclick={onClick}
	aria-label={m.emoji()}
	data-testid="message-input-emoji"
>
	<wa-icon style="font-size: 26px" src={wrapPathInSvg(mdiEmoticonHappyOutline)}
	></wa-icon>
</button>

<style>
	.icon-button {
		flex-shrink: 0;
		width: 28px;
		height: 28px;
		border: none;
		background: transparent;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.5;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}

	.icon-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.1);
	}

	.icon-button:active {
		background: rgba(128, 128, 128, 0.2);
	}

	.icon-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}
</style>
