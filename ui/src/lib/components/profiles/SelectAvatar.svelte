<script lang="ts">
	import { resizeAndExport } from '$lib/utils/image';
	import { onActivate } from '$lib/utils/keyboard';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccount } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import Avatar from './Avatar.svelte';

	let {
		defaultValue,
		value = $bindable(),
		size = 46,
		placeholderIconPath = mdiAccount,
		placeholderLabel = m.addAvatarImage(),
	}: {
		value?: string | undefined;
		defaultValue?: string | undefined;
		size?: number;
		placeholderIconPath?: string;
		placeholderLabel?: string;
	} = $props();
	let uploading = $state(false);
	let avatarFilePicker: HTMLInputElement;

	function onAvatarUploaded() {
		uploading = true;
		if (avatarFilePicker.files && avatarFilePicker.files[0]) {
			const reader = new FileReader();
			reader.onload = e => {
				const img = new Image();
				img.crossOrigin = 'anonymous';
				img.onload = () => {
					value = resizeAndExport(img);
					avatarFilePicker.value = '';

					uploading = false;
				};
				img.src = e.target?.result as string;
			};
			reader.readAsDataURL(avatarFilePicker.files[0]);
		}
	}
</script>

<input
	type="file"
	bind:this={avatarFilePicker}
	style="display: none"
	onchange={onAvatarUploaded}
/>

{#if value}
	<div
		class="column"
		style="align-items: center; height: {size + 4}px"
		role="button"
		tabindex="0"
		onclick={() => avatarFilePicker.click()}
		onkeydown={onActivate(() => avatarFilePicker.click())}
	>
		<Avatar id="avatar" image={value} alt="Avatar" style="--size: {size}px" />
	</div>
{:else if defaultValue}
	<div
		class="column"
		style="align-items: center; height: {size + 4}px"
		role="button"
		tabindex="0"
		onclick={() => avatarFilePicker.click()}
		onkeydown={onActivate(() => avatarFilePicker.click())}
	>
		<Avatar
			id="avatar"
			image={defaultValue}
			alt="Avatar"
			style="--size: {size}px"
		/>
	</div>
{:else}
	<button
		type="button"
		onclick={() => avatarFilePicker.click()}
		disabled={uploading}
		aria-label={placeholderLabel}
		class="rounded-full flex items-center justify-center bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-200 disabled:opacity-50"
		style="height: {size}px; width: {size}px"
	>
		<wa-icon
			src={wrapPathInSvg(placeholderIconPath)}
			label={placeholderLabel}
			style="font-size: {Math.round(size * 0.5)}px;"
		></wa-icon>
	</button>
{/if}
