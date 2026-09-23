<script lang="ts" module>
	export interface SearchState {
		focused: boolean;
		query: string;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	// @ts-ignore
	import { Picker } from 'emoji-picker-element'; // missing type definition

	interface Props {
		onEmojiSelected: (emoji: string) => void;
		onSearchChange?: (search: SearchState) => void;
	}
	let { onEmojiSelected, onSearchChange }: Props = $props();

	let content: Element;
	let pickerComponent: HTMLElement | undefined;
	let search: SearchState = { focused: false, query: '' };

	// Deferred: focus events also fire while Svelte is tearing the picker down,
	// where a listener must not write component state.
	function reportSearch(change: Partial<SearchState>) {
		search = { ...search, ...change };
		const reported = search;
		queueMicrotask(() => onSearchChange?.(reported));
	}

	function searchInput() {
		return pickerComponent?.shadowRoot?.querySelector<HTMLInputElement>(
			'input[type="search"]',
		);
	}

	export function blurSearch() {
		searchInput()?.blur();
	}

	export function clearSearch() {
		const input = searchInput();
		if (!input) return;
		input.blur();
		if (input.value === '') return;
		input.value = '';
		input.dispatchEvent(new Event('input', { bubbles: true }));
	}

	function searchInputOf(event: Event) {
		const target = event.target;
		return target instanceof HTMLInputElement && target.type === 'search'
			? target
			: null;
	}

	onMount(() => {
		const picker = new Picker({
			// if not set the library will try and fetch online
			// i18n requires having one of these per language
			// from https://cdn.jsdelivr.net/npm/emoji-picker-element-data@^1/en/emojibase/data.json
			dataSource: '/emoji.en.json',
		});
		picker.addEventListener('emoji-click', event => {
			if (event.detail.unicode) {
				onEmojiSelected(event.detail.unicode);
			}
		});
		// On the shadow root, not the host: focus moving between elements inside
		// the picker (search → category tab) never reaches the host.
		const shadowRoot = picker.shadowRoot!;
		shadowRoot.addEventListener('focusin', (event: Event) => {
			if (searchInputOf(event)) reportSearch({ focused: true });
		});
		shadowRoot.addEventListener('focusout', (event: Event) => {
			if (searchInputOf(event)) reportSearch({ focused: false });
		});
		shadowRoot.addEventListener('input', (event: Event) => {
			const input = searchInputOf(event);
			if (input) reportSearch({ query: input.value });
		});
		content.appendChild(picker);
		pickerComponent = picker;
	});
</script>

<div bind:this={content} class="h-full w-full"></div>
