<script lang="ts">
	import { onMount } from 'svelte';
	// @ts-ignore
	import { Picker } from 'emoji-picker-element'; // missing type definition

	interface Props {
		onEmojiSelected: (emoji: string) => void;
	}
	let { onEmojiSelected }: Props = $props();

	let content: Element;
	let pickerComponent: HTMLElement | undefined;

	/** Empty the search and take focus off it, which also drops its keyboard. */
	export function clearSearch() {
		const input =
			pickerComponent?.shadowRoot?.querySelector<HTMLInputElement>('#search');
		if (!input) return;
		input.blur();
		if (input.value === '') return;
		input.value = '';
		input.dispatchEvent(new Event('input', { bubbles: true }));
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
		content.appendChild(picker);
		pickerComponent = picker;
	});
</script>

<div bind:this={content} class="h-full w-full"></div>
