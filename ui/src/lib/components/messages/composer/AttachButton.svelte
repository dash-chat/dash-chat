<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPlus, mdiClose } from '@mdi/js';

	interface Props {
		onClick: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
		testid?: string;
		class?: string;
	}

	let {
		onClick,
		expanded = false,
		testid = 'message-input-attach',
		class: className = '',
	}: Props = $props();
</script>

<button
	type="button"
	class="attach-button flex shrink-0 items-center justify-center {className}"
	data-testid={testid}
	aria-label={m.attachMenu()}
	aria-expanded={expanded}
	onclick={onClick}
>
	<wa-icon src={wrapPathInSvg(expanded ? mdiClose : mdiPlus)}></wa-icon>
</button>

<style>
	.attach-button {
		border: none;
		background: transparent;
		border-radius: 50%;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.6;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}
	.attach-button:hover {
		opacity: 0.85;
		background: rgba(128, 128, 128, 0.1);
	}
	.attach-button:active {
		background: rgba(128, 128, 128, 0.2);
	}
	.attach-button :global(wa-icon) {
		width: 24px;
		height: 24px;
	}
</style>
