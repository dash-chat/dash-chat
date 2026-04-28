<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend, mdiEmoticonHappyOutline } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { onMount } from 'svelte';
	import { isIos } from '$lib/utils/environment';

	interface Props {
		value?: string;
		placeholder?: string;
		height: string;
		onSend?: () => void;
		onInput?: () => void;
		onFocus?: () => { onResize: () => void } | void;
		onEmojiClick?: () => void;
	}

	let {
		value = $bindable(''),
		height = $bindable(''),
		placeholder = m.typeMessage(),
		onSend,
		onInput,
		onFocus,
		onEmojiClick,
	}: Props = $props();
	let div: HTMLDivElement;

	const theme = $derived(useTheme());

	let hasText = $derived(value.trim().length > 0);
	let textarea: HTMLTextAreaElement;

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			triggerOnSend();
		}
	}

	function handleInput() {
		value = textarea.value;
		autoResize();
		onInput?.();
	}

	function autoResize() {
		if (textarea.scrollHeight > 100) return;
		textarea.style.height = 'auto';
		const textareaHeight = textarea.scrollHeight + 'px';
		textarea.style.height = textareaHeight;
		height = `${div.scrollHeight}px`;
	}

	function handleSendClick() {
		triggerOnSend();
	}

	function triggerOnSend() {
		if (hasText) {
			onSend?.();
			textarea.style.height = 'auto';
			height = `${div.scrollHeight}px`;
		}
	}

	function handleFocus() {
		const result = onFocus?.();
		if (!result) return;
		const vv = window.visualViewport;
		if (!vv) return;
		let cleanup: ReturnType<typeof setTimeout>;
		const onResize = () => {
			result.onResize();
			clearTimeout(cleanup);
			cleanup = setTimeout(
				() => vv.removeEventListener('resize', onResize),
				300,
			);
		};
		vv.addEventListener('resize', onResize);
	}

	onMount(() => {
		height = `${div.scrollHeight}px`;
	});
</script>

<div
	bind:this={div}
	class="message-input-bar m-2 pb-safe"
	class:bg-md-light-surface={theme === 'material'}
	class:dark:bg-md-dark-surface={theme === 'material'}
>
	<div class="row gap-2" style="align-items: flex-end; margin: 0 auto">
		<div
			class={theme === 'ios'
				? 'input-container bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
				: 'input-container bg-white dark:bg-gray-800'}
		>
			{#if onEmojiClick && !isIos}
				<button
					type="button"
					class="icon-button emoji-btn"
					onclick={onEmojiClick}
					aria-label="Emoji"
					data-testid="message-input-emoji"
				>
					<wa-icon src={wrapPathInSvg(mdiEmoticonHappyOutline)}></wa-icon>
				</button>
			{/if}

			<textarea
				class="message-textarea"
				data-testid="message-input-textarea"
				{placeholder}
				bind:value
				bind:this={textarea}
				rows="1"
				onkeydown={handleKeydown}
				oninput={handleInput}
				onfocus={handleFocus}
			></textarea>
		</div>

		<button
			type="button"
			class="send-button"
			data-testid="message-input-send"
			class:active={hasText}
			onclick={handleSendClick}
			disabled={!hasText}
			aria-label="Send"
		>
			<wa-icon src={wrapPathInSvg(mdiSend)}></wa-icon>
		</button>
	</div>
</div>

<style>
	.input-container {
		flex: 1;
		display: flex;
		align-items: flex-end;
		min-width: 0;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		padding: 4px 4px 4px 6px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--k-theme-color, #3b82f6);
	}

	.icon-button {
		flex-shrink: 0;
		width: 36px;
		height: 36px;
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
		padding: 0;
	}

	.icon-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.1);
	}

	.icon-button:active {
		background: rgba(128, 128, 128, 0.2);
	}

	.message-textarea {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		resize: none;
		font-size: 16px;
		line-height: 1.375;
		padding: 8px 8px;
		color: var(--k-text-color);
		font-family: inherit;
		min-height: 20px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}

	.send-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		margin-bottom: 4px;
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

	.send-button.active {
		background: var(--k-theme-color, #3b82f6);
		color: white;
		opacity: 1;
	}

	.send-button.active:hover {
		filter: brightness(1.1);
	}

	.send-button.active:active {
		transform: scale(0.95);
	}

	/* Icon sizing */
	.icon-button :global(wa-icon),
	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.send-button :global(wa-icon) {
		margin-left: 2px; /* Optical centering for send arrow */
	}
</style>
