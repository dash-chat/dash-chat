<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiCamera, mdiImage, mdiArrowLeft } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Link, Navbar, Segmented, SegmentedButton } from 'konsta/svelte';
	import { resizeAndExport } from '$lib/utils/image';
	import { isMobile } from '$lib/utils/environment';

	let {
		avatar = $bindable(),
		isTextEditorOpen = $bindable(false),
		onSelect,
		onClose,
	}: {
		avatar?: string | undefined;
		isTextEditorOpen?: boolean;
		onSelect?: () => void;
		onClose?: () => void;
	} = $props();

	let view = $state<'picker' | 'text'>('picker');
	$effect(() => {
		isTextEditorOpen = view === 'text';
	});
	let textValue = $state('');
	let selectedColor = $state('#fce7f3');
	let activeTab = $state<'text' | 'color'>('text');
	let hiddenInput: HTMLInputElement;
	let avatarFilePicker: HTMLInputElement;

	const defaultAvatars = [
		'🐸', '🐱', '🐶', '🦊',
		'🐻', '🐼', '🦁', '🐷',
		'🐧', '🦉', '🐢', '🦄',
		'👻', '🐰', '🐮', '🐵',
	];

	const colors = [
		'#ddd6fe', '#bfdbfe', '#cffafe', '#bbf7d0',
		'#e9d5ff', '#fbcfe8', '#fce7f3', '#fecaca',
		'#fef08a', '#d9f99d', '#e5e7eb', '#d1d5db',
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
		setTimeout(() => hiddenInput?.focus(), 100);
	}

	function generateTextAvatar() {
		const canvas = document.createElement('canvas');
		canvas.width = 256;
		canvas.height = 256;
		const ctx = canvas.getContext('2d')!;

		ctx.fillStyle = selectedColor;
		ctx.beginPath();
		ctx.arc(128, 128, 128, 0, Math.PI * 2);
		ctx.fill();

		ctx.fillStyle = '#831843';
		ctx.font = '500 100px sans-serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText(textValue.toUpperCase(), 128, 135);

		avatar = canvas.toDataURL('image/png');
		view = 'picker';
		onSelect?.();
	}

	function handleTextInput(e: Event) {
		const input = e.target as HTMLInputElement;
		textValue = input.value.slice(0, 3).toUpperCase();
	}

	function focusTextInput() {
		if (activeTab === 'text') {
			hiddenInput?.focus();
		}
	}

	$effect(() => {
		if (view === 'text' && activeTab === 'text') {
			setTimeout(() => hiddenInput?.focus(), 100);
		}
	});
</script>

<input
	type="file"
	accept="image/*"
	bind:this={avatarFilePicker}
	style="display: none"
	onchange={onAvatarUploaded}
/>

<input
	type="text"
	class="absolute opacity-0 pointer-events-none"
	bind:this={hiddenInput}
	value={textValue}
	oninput={handleTextInput}
	maxlength="3"
	onblur={() =>
		view === 'text' &&
		activeTab === 'text' &&
		setTimeout(() => hiddenInput?.focus(), 0)}
/>

{#if view === 'picker'}
	{#if onClose}
		<Navbar transparent>
			{#snippet left()}
				<Link iconOnly onClick={onClose}>
					<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 24px"></wa-icon>
				</Link>
			{/snippet}
		</Navbar>
	{/if}

	<!-- Avatar preview with remove button -->
	<div class="column" style="align-items: center; padding: 16px 0 24px;">
		<div style="position: relative; display: inline-block;">
			<wa-avatar style="--size: 140px" image={avatar}></wa-avatar>
			{#if avatar}
				<button
					class="absolute top-2 right-2 w-10 h-10 rounded-[10px] bg-white text-gray-700 border-none cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-gray-100 dark:bg-gray-600 dark:text-white dark:hover:bg-gray-500"
					onclick={removeAvatar}
					aria-label={m.removePhoto()}
				>
					<wa-icon
						src={wrapPathInSvg(mdiClose)}
						style="font-size: 20px"
					></wa-icon>
				</button>
			{/if}
		</div>
	</div>

	<!-- Action buttons: Camera, Photo, Text -->
	<div
		class="row gap-4"
		style="justify-content: center; padding: 0 16px 24px;"
	>
		{#if isMobile}
			<div class="column" style="align-items: center; gap: 8px;">
				<Button
					tonal
					onClick={() => avatarFilePicker.click()}
					class="icon-only"
				>
					<wa-icon
						src={wrapPathInSvg(mdiCamera)}
						style="font-size: 28px"
					></wa-icon>
				</Button>
				<span class="text-sm" style="color: var(--k-text-color)">{m.camera()}</span>
			</div>
		{/if}

		<div class="column" style="align-items: center; gap: 8px;">
			<Button
				tonal
				onClick={() => avatarFilePicker.click()}
				class="icon-only"
			>
				<wa-icon
					src={wrapPathInSvg(mdiImage)}
					style="font-size: 28px"
				></wa-icon>
			</Button>
			<span class="text-sm" style="color: var(--k-text-color)">{m.photo()}</span>
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
	<!-- Text avatar editor -->
	<Navbar transparent>
		{#snippet left()}
			<Link iconOnly onClick={() => (view = 'picker')} data-testid="edit-photo-back">
				<wa-icon src={wrapPathInSvg(mdiArrowLeft)} style="font-size: 24px"></wa-icon>
			</Link>
		{/snippet}
	</Navbar>

	<div style="padding: 0 16px 16px;">
		<Segmented strong>
			<SegmentedButton
				active={activeTab === 'text'}
				onClick={() => (activeTab = 'text')}
			>
				{m.text()}
			</SegmentedButton>
			<SegmentedButton
				active={activeTab === 'color'}
				onClick={() => (activeTab = 'color')}
			>
				{m.color()}
			</SegmentedButton>
		</Segmented>
	</div>

	<!-- Text avatar preview -->
	<div class="column" style="align-items: center; padding: 24px 0;">
		<button
			class="w-[180px] h-[180px] rounded-full flex items-center justify-center border-none cursor-pointer"
			style="background-color: {selectedColor};"
			onclick={focusTextInput}
			type="button"
		>
			{#if activeTab === 'text'}
				<span class="text-[56px] font-medium text-pink-900"
					>{textValue}<span class="text-[56px] font-light text-pink-900 animate-[blink_1s_infinite] -ml-0.5">|</span></span
				>
			{:else}
				<span class="text-[56px] font-medium text-pink-900">{textValue}</span>
			{/if}
		</button>
	</div>

	{#if activeTab === 'color'}
		<div class="grid grid-cols-4 gap-4 px-6 py-6 justify-items-center">
			{#each colors as color}
				<button
					class="w-[72px] h-[72px] rounded-full border-[3px] cursor-pointer transition-transform duration-200 hover:scale-105 active:scale-95 {selectedColor === color ? 'border-gray-700' : 'border-transparent'}"
					style="background-color: {color};"
					onclick={() => (selectedColor = color)}
				>
				</button>
			{/each}
		</div>
	{/if}

	<Button
		rounded
		tonal
		disabled={!textValue}
		onClick={generateTextAvatar}
				class="fixed-action-btn"
	>
		{m.done()}
	</Button>
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
