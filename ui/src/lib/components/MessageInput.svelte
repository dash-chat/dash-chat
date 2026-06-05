<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend, mdiEmoticonHappyOutline } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { isIos, isMobile } from '$lib/utils/environment';

	interface Props {
		value?: string;
		placeholder?: string;
		onSend?: () => void;
		onEmojiClick?: () => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		onSend,
		onEmojiClick,
	}: Props = $props();

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
	}

	function autoResize() {
		if (textarea.scrollHeight > 100) return;
		textarea.style.height = 'auto';
		textarea.style.height = textarea.scrollHeight + 'px';
	}

	function handleSendClick() {
		triggerOnSend();
	}

	function triggerOnSend() {
		if (hasText) {
			onSend?.();
			textarea.style.height = 'auto';
			textarea.focus();
		}
	}

	function keepKeyboardOpen(event: Event) {
		if (event.target !== textarea) {
			event.preventDefault();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	style="display: flow-root"
	onmousedown={keepKeyboardOpen}
	ontouchstart={keepKeyboardOpen}
	onpointerdown={keepKeyboardOpen}
>
	<div
		class="message-input-bar m-2 pb-safe"
		class:bg-md-light-surface={theme === 'material'}
		class:dark:bg-md-dark-surface={theme === 'material'}
	>
		<div class="row gap-2" style="align-items: flex-end; margin: 0 auto">
			<div
				class={theme === 'ios'
					? 'input-container bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'input-container bg-white dark:bg-gray-800'}
				style="padding-left: 8px"
			>
				{#if onEmojiClick && !isIos}
					<button
						type="button"
						class="icon-button emoji-btn"
						onclick={onEmojiClick}
						aria-label="Emoji"
						data-testid="message-input-emoji"
					>
						<wa-icon
							style="font-size: 26px"
							src={wrapPathInSvg(mdiEmoticonHappyOutline)}
						></wa-icon>
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
				></textarea>
			</div>

			{#if isMobile}
				<button
					type="button"
					class="send-button"
					data-testid="message-input-send"
					class:active={hasText}
					onclick={handleSendClick}
					disabled={!hasText}
					aria-label="Send"
				>
					<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}
					></wa-icon>
				</button>
			{/if}
		</div>
	</div>
</div>

<style>
	.input-container {
		flex: 1;
		display: flex;
		align-items: center;
		min-width: 0;
		min-height: 42px;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--k-theme-color, #3b82f6);
	}

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

	.message-textarea {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		resize: none;
		line-height: 1.375;
		color: var(--k-text-color);
		font-family: inherit;
		min-height: 28px;
		padding: 8px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}

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
		margin-inline-start: 2px; /* Optical centering for send arrow */
	}
</style>
