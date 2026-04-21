<script lang="ts">
	import { resizeAndExport } from '$lib/utils/image';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccount } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button } from 'konsta/svelte';
	import Avatar from './Avatar.svelte';

	let {
		value = $bindable(),
		defaultValue,
		size = 46,
	}: {
		value?: string | undefined;
		defaultValue?: string | undefined;
		size?: number;
	} = $props();
	let uploading = $state(false);
	let avatarFilePicker: HTMLInputElement;

	if (!value) {
		value = defaultValue;
	}

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
		onclick={() => avatarFilePicker.click()}
	>
		<Avatar
			id="avatar"
			image={value}
			alt="Avatar"
			initials=""
			style="--size: {size}px"
		/>
	</div>
{:else if defaultValue}
	<div
		class="column"
		style="align-items: center; height: {size + 4}px"
		onclick={() => avatarFilePicker.click()}
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
	<div class="column" style="align-items: center; height: {size + 4}px">
		<Button
			onclick={() => avatarFilePicker.click()}
			disabled={uploading}
			rounded
			style="border-radius: 50%; height: {size}px; width: {size}px"
		>
			<wa-icon src={wrapPathInSvg(mdiAccount)} label={m.addAvatarImage()}
			></wa-icon>
		</Button>
	</div>
{/if}
