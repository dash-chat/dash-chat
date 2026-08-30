<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { useTheme } from 'konsta/svelte';
	import { showKeyboard } from 'tauri-plugin-virtual-keyboard';

	interface Props {
		value?: string;
		placeholder?: string;
		onSend?: () => Promise<boolean>;
		onpaste?: (event: ClipboardEvent) => void;
		onfocus?: (event: FocusEvent) => void;
		/** The composer is hiding this row behind another surface (e.g. the voice
		 * recording bar). */
		hidden?: boolean;
		/** Leading content rendered inside the pill, before the textarea. */
		before?: Snippet;
		/** Trailing content rendered inside the pill, after the textarea. */
		after?: Snippet;
		/** Full-width row rendered inside the pill, above the textarea (e.g.
		 * the editing banner). */
		banner?: Snippet;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		onSend,
		onpaste,
		onfocus,
		hidden = false,
		before,
		after,
		banner,
	}: Props = $props();

	const theme = $derived(useTheme());

	let textarea: HTMLTextAreaElement;

	export function reset() {
		textarea.style.height = 'auto';
	}

	/** Focus the input with the cursor at the end, sized to the current text. */
	export function focus() {
		textarea.focus();
		// Android suppresses the IME on programmatic focus outside a tap
		// gesture (e.g. swipe-to-reply), showing it only after a long delay.
		showKeyboard();
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

<div
	class="input-container flex min-h-[42px] min-w-0 flex-1 flex-col justify-center {theme ===
	'ios'
		? 'input-container-border bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
		: 'bg-incoming-surface'}"
	{onpaste}
>
	{@render banner?.()}

	<div class="flex w-full items-center">
		{@render before?.()}

		<textarea
			class:ms-4={!before}
			class:blanked={hidden}
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
</div>

<style>
	.input-container {
		border-radius: 22px;
	}

	.input-container-border {
		border: 1px solid var(--k-hairline-color);
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

	/* The composer hides the whole input row while another surface covers it, but
	   hiding the focused textarea blurs it and takes the keyboard down with it.
	   Stay rendered and keep focus; just paint nothing. */
	.message-textarea.blanked {
		visibility: visible;
		opacity: 0;
		pointer-events: none;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}
</style>
