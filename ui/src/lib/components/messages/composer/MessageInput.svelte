<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { useTheme } from 'konsta/svelte';
	import { showKeyboard } from 'tauri-plugin-virtual-keyboard';
	import { isMobile } from '$lib/utils/environment';

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

	/** Focus the input with the cursor at the end. */
	export function focus() {
		textarea.focus();
		// Android suppresses the IME on programmatic focus outside a tap
		// gesture (e.g. swipe-to-reply), showing it only after a long delay.
		showKeyboard();
		textarea.setSelectionRange(textarea.value.length, textarea.value.length);
	}

	function handleKeydown(event: KeyboardEvent) {
		// On a soft keyboard the return key is the only way to type a line break,
		// and the send button is always at hand.
		if (isMobile) return;
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			onSend?.();
		}
	}

	let heightAnimation: Animation | undefined;

	// Follow every value change, not just typing: the composer also writes it
	// programmatically (starting/cancelling an edit, sending, picking an emoji),
	// and a stale inline height leaves the pill stuck at its previous size.
	$effect(() => {
		value;
		const before = textarea.getBoundingClientRect().height;
		textarea.style.height = 'auto';
		textarea.style.height = textarea.scrollHeight + 'px';
		if (theme !== 'ios') return;
		const after = textarea.getBoundingClientRect().height;
		if (before === after) return;
		// A CSS transition can't do this: resetting to `auto` to measure lands the
		// element on the new height before the final write, so there is nothing
		// left to interpolate. Replay it from the old height instead.
		heightAnimation?.cancel();
		heightAnimation = textarea.animate(
			[{ height: `${before}px` }, { height: `${after}px` }],
			{ duration: 250, easing: 'cubic-bezier(0.25, 0.1, 0.25, 1)' },
		);
	});
</script>

<div
	class="input-container flex min-w-0 flex-1 flex-col justify-center {theme ===
	'ios'
		? 'input-container-border min-h-[42px] bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
		: 'min-h-[44px] bg-incoming-surface'}"
	{onpaste}
>
	{@render banner?.()}

	<div class="relative flex w-full items-end" class:pe-1={after}>
		{@render before?.()}

		<textarea
			class:ms-4={!before}
			class:me-4={!after}
			class:me-2={after}
			class:blanked={hidden}
			class="message-textarea self-center"
			data-testid="message-input-textarea"
			{placeholder}
			bind:value
			bind:this={textarea}
			rows="1"
			onkeydown={handleKeydown}
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
