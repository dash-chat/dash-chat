<script lang="ts">
	import { onMount } from 'svelte';
	// @ts-ignore
	import { Picker } from 'emoji-picker-element'; // missing type definition

	interface Props {
		onEmojiSelected: (emoji: string) => void;
	}
	let { onEmojiSelected }: Props = $props();

	let content: Element;

	onMount(() => {
		let pickerComponent = new Picker({
			// if not set the library will try and fetch online
			// i18n requires having one of these per language
			// from https://cdn.jsdelivr.net/npm/emoji-picker-element-data@^1/en/emojibase/data.json
			dataSource: '/emoji.en.json',
		});
		pickerComponent.addEventListener('emoji-click', event => {
			if (event.detail.unicode) {
				onEmojiSelected(event.detail.unicode);
			}
		});
		content.appendChild(pickerComponent);
	});
</script>

<div bind:this={content} class="w-full"></div>
