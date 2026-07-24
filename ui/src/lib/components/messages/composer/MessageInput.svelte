<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { useTheme } from 'konsta/svelte';

	interface Props {
		value?: string;
		placeholder?: string;
		onSend?: () => Promise<boolean>;
		onpaste?: (event: ClipboardEvent) => void;
		onfocus?: (event: FocusEvent) => void;
		/** Leading content rendered inside the pill, before the textarea. */
		before?: Snippet;
		/** Trailing content rendered inside the pill, after the textarea. */
		after?: Snippet;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		onSend,
		onpaste,
		onfocus,
		before,
		after,
	}: Props = $props();

	const theme = $derived(useTheme());

	let textarea: HTMLTextAreaElement;

	export function reset() {
		textarea.style.height = 'auto';
	}

	export function focus() {
		textarea.focus();
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

<div
	class="input-container flex min-h-[42px] min-w-0 flex-1 items-center {theme ===
	'ios'
		? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
		: 'bg-white dark:bg-gray-800'}"
	{onpaste}
>
	{@render before?.()}

	<textarea
		class:ms-4={!before}
		class="message-textarea me-2"
		data-testid="message-input-textarea"
		{placeholder}
		bind:value
		bind:this={textarea}
		rows="1"
		onkeydown={handleKeydown}
		oninput={handleInput}
		{onfocus}
	></textarea>

	{@render after?.()}
</div>

<style>
	.input-container {
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
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
		padding-top: 8px;
		padding-bottom: 8px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}
</style>
