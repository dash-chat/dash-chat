<script lang="ts">
	import TextAvatarPicker from './TextAvatarPicker.svelte';

	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiCamera, mdiImage } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Link, Navbar } from 'konsta/svelte';
	import { resizeAndExport } from '$lib/utils/image';
	import { isMobile, isIos } from '$lib/utils/environment';
	import Avatar from './Avatar.svelte';

	let {
		avatar = $bindable(),
		inModalState = $bindable(false),
		onSelect,
		onClose,
		onSave,
		saveLabel,
		saveDisabled = false,
	}: {
		avatar?: string | undefined;
		inModalState?: boolean;
		onSelect?: () => void;
		onClose?: () => void;
		onSave?: () => void;
		saveLabel?: string;
		saveDisabled?: boolean;
	} = $props();

	let view = $state<'picker' | 'text'>('picker');
	$effect(() => {
		inModalState = view === 'text';
	});
	let textValue = $state('');
	let selectedColor = $state('#fce7f3');
	let activeTab = $state<'text' | 'color'>('text');
	let hiddenInput: HTMLInputElement;
	let avatarFilePicker: HTMLInputElement;

	const defaultAvatars = [
		'🐸',
		'🐱',
		'🐶',
		'🦊',
		'🐻',
		'🐼',
		'🦁',
		'🐷',
		'🐧',
		'🦉',
		'🐢',
		'🦄',
		'👻',
		'🐰',
		'🐮',
		'🐵',
	];

	function onAvatarUploaded() {
		if (avatarFilePicker.files && avatarFilePicker.files[0]) {
			const reader = new FileReader();
			reader.onload = e => {
				const img = new Image();
				img.crossOrigin = 'anonymous';
				img.onload = () => {
					avatar = resizeAndExport(img);
					avatarFilePicker.value = '';
					onSelect?.();
				};
				img.src = e.target?.result as string;
			};
			reader.readAsDataURL(avatarFilePicker.files[0]);
		}
	}

	function removeAvatar() {
		avatar = undefined;
	}

	function selectDefaultAvatar(emoji: string) {
		const canvas = document.createElement('canvas');
		canvas.width = 256;
		canvas.height = 256;
		const ctx = canvas.getContext('2d')!;

		ctx.fillStyle = '#e5e7eb';
		ctx.beginPath();
		ctx.arc(128, 128, 128, 0, Math.PI * 2);
		ctx.fill();

		ctx.font =
			'140px "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText(emoji, 128, 128);

		avatar = canvas.toDataURL('image/png');
		onSelect?.();
	}

	function openTextEditor() {
		textValue = '';
		selectedColor = '#fce7f3';
		activeTab = 'text';
		view = 'text';
		// setTimeout(() => hiddenInput?.focus(), 100);
	}

	// function handleTextInput(e: Event) {
	// 	const input = e.target as HTMLInputElement;
	// 	textValue = input.value.slice(0, 3).toUpperCase();
	// }

	// function focusTextInput() {
	// 	if (activeTab === 'text') {
	// 		hiddenInput?.focus();
	// 	}
	// }

	// $effect(() => {
	// 	if (view === 'text' && activeTab === 'text') {
	// 		setTimeout(() => hiddenInput?.focus(), 100);
	// 	}
	// });
</script>

<input
	type="file"
	accept="image/*"
	bind:this={avatarFilePicker}
	class="hidden"
	onchange={onAvatarUploaded}
/>

{#if view === 'picker'}
	{#if onClose || (onSave && isIos)}
		<Navbar transparent rightClass={saveDisabled ? 'ios-right-disabled' : ''}>
			{#snippet left()}
				{#if onClose}
					<Link iconOnly onClick={onClose} data-testid="edit-photo-close">
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 24px"
						></wa-icon>
					</Link>
				{/if}
			{/snippet}
			{#snippet right()}
				{#if isIos && onSave}
					<Link onClick={onSave} data-testid="edit-photo-save-link">
						{saveLabel || m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>
	{/if}

	<!-- Avatar preview with remove button -->
	<div class="column" style="align-items: center; padding: 16px 0 24px;">
		<div style="position: relative; display: inline-block;">
			<Avatar style="--size: 140px" image={avatar} />
			{#if avatar}
				<button
					class="absolute top-2 right-2 w-10 h-10 rounded-[10px] bg-white text-gray-700 border-none cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-gray-100 dark:bg-gray-600 dark:text-white dark:hover:bg-gray-500"
					onclick={removeAvatar}
					aria-label={m.removePhoto()}
				>
					<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 20px"
					></wa-icon>
				</button>
			{/if}
		</div>
	</div>

	<!-- Action buttons: Camera, Photo, Text -->
	<div class="row gap-4" style="justify-content: center; padding: 0 16px 24px;">
		{#if isMobile}
			<div class="column" style="align-items: center; gap: 8px;">
				<Button
					tonal
					onClick={() => avatarFilePicker.click()}
					class="icon-only"
				>
					<wa-icon src={wrapPathInSvg(mdiCamera)} style="font-size: 28px"
					></wa-icon>
				</Button>
				<span class="text-sm" style="color: var(--k-text-color)"
					>{m.camera()}</span
				>
			</div>
		{/if}

		<div class="column" style="align-items: center; gap: 8px;">
			<Button tonal onClick={() => avatarFilePicker.click()} class="icon-only">
				<wa-icon src={wrapPathInSvg(mdiImage)} style="font-size: 28px"
				></wa-icon>
			</Button>
			<span class="text-sm" style="color: var(--k-text-color)">{m.photo()}</span
			>
		</div>

		<div class="column" style="align-items: center; gap: 8px;">
			<Button
				tonal
				onClick={openTextEditor}
				class="icon-only"
				style="font-size: 20px; font-weight: 600"
			>
				Aa
			</Button>
			<span class="text-sm" style="color: var(--k-text-color)">{m.text()}</span>
		</div>
	</div>

	<!-- Divider -->
	<div style="height: 1px; background: var(--k-hairline-color);"></div>

	<!-- Default avatars grid -->
	<div class="grid grid-cols-4 gap-3 py-6 px-4 justify-items-center">
		{#each defaultAvatars as emoji}
			<button
				class="w-[72px] h-[72px] rounded-full bg-gray-200 border-none cursor-pointer flex items-center justify-center transition-all duration-200 hover:scale-105 hover:bg-gray-300 active:scale-95"
				onclick={() => selectDefaultAvatar(emoji)}
			>
				<span style="font-size: 32px">{emoji}</span>
			</button>
		{/each}
	</div>
{:else}
	<TextAvatarPicker
		bind:avatar
		onSelect={() => (view = 'picker')}
		onClose={() => (view = 'picker')}
	/>
{/if}

<style>
	@keyframes blink {
		0%,
		50% {
			opacity: 1;
		}
		51%,
		100% {
			opacity: 0;
		}
	}
</style>
