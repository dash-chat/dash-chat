<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { isIos } from '$lib/utils/environment';
	import EmojiButton from '$lib/components/messages/composer/EmojiButton.svelte';

	interface Props {
		value?: string;
		placeholder?: string;
		onSend?: () => Promise<boolean>;
		onEmojiClick?: () => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		onSend,
		onEmojiClick,
	}: Props = $props();

	let textarea: HTMLTextAreaElement;

	export function reset() {
		textarea.style.height = 'auto';
	}

	/** Focus the input with the cursor at the end, sized to the current text. */
	export function focus() {
		textarea.focus();
		textarea.setSelectionRange(textarea.value.length, textarea.value.length);
		autoResize();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			onSend?.();
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
</script>

{#if onEmojiClick && !isIos}
	<EmojiButton onClick={onEmojiClick} />
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

<style>
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
</style>
