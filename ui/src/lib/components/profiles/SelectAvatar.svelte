<script lang="ts">
	import { fileToAvatar } from '$lib/utils/image';
	import { pickMedia } from '$lib/utils/media';
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

	async function selectAvatar() {
		const file = (await pickMedia('image', false))?.[0];
		if (!file) return;
		uploading = true;
		try {
			value = await fileToAvatar(file);
		} finally {
			uploading = false;
		}
	}
</script>

{#if value}
	<div
		class="column"
		style="align-items: center; height: {size + 4}px"
		role="button"
		tabindex="0"
		onclick={selectAvatar}
		onkeydown={onActivate(selectAvatar)}
	>
		<Avatar id="avatar" image={value} alt="Avatar" initials="" {size} />
	</div>
{:else if defaultValue}
	<div
		class="column"
		style="align-items: center; height: {size + 4}px"
		role="button"
		tabindex="0"
		onclick={selectAvatar}
		onkeydown={onActivate(selectAvatar)}
	>
		<Avatar
			id="avatar"
			image={defaultValue}
			alt="Avatar"
			initials=""
			style="--size: {size}px"
		/>
	</div>
{:else}
	<button
		type="button"
		onclick={selectAvatar}
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
