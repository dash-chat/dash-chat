<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiCamera, mdiImage, mdiArrowLeft } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Segmented, SegmentedButton } from 'konsta/svelte';
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
	class="hidden-input"
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
		<div class="nav-header">
			<button
				class="nav-btn"
				onclick={onClose}
				aria-label="Close"
			>
				<wa-icon
					src={wrapPathInSvg(mdiClose)}
					style="font-size: 28px"
				></wa-icon>
			</button>
		</div>
	{/if}

	<!-- Avatar preview with remove button -->
	<div class="column" style="align-items: center; padding: 16px 0 24px;">
		<div style="position: relative; display: inline-block;">
			<wa-avatar style="--size: 140px" image={avatar}></wa-avatar>
			{#if avatar}
				<button
					class="remove-avatar-btn"
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
				<span class="action-label">{m.camera()}</span>
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
			<span class="action-label">{m.photo()}</span>
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
			<span class="action-label">{m.text()}</span>
		</div>
	</div>

	<!-- Divider -->
	<div style="height: 1px; background: var(--k-hairline-color);"></div>

	<!-- Default avatars grid -->
	<div class="avatar-grid">
		{#each defaultAvatars as emoji}
			<button
				class="default-avatar-btn"
				onclick={() => selectDefaultAvatar(emoji)}
			>
				<span style="font-size: 32px">{emoji}</span>
			</button>
		{/each}
	</div>
{:else}
	<!-- Text avatar editor -->
	<div class="nav-header">
		<button
			class="nav-btn"
			onclick={() => (view = 'picker')}
			aria-label="Back"
		>
			<wa-icon
				src={wrapPathInSvg(mdiArrowLeft)}
				style="font-size: 24px"
			></wa-icon>
		</button>
	</div>

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
			class="text-avatar-preview"
			style="background-color: {selectedColor};"
			onclick={focusTextInput}
			type="button"
		>
			{#if activeTab === 'text'}
				<span class="avatar-text"
					>{textValue}<span class="avatar-cursor">|</span></span
				>
			{:else}
				<span class="avatar-text">{textValue}</span>
			{/if}
		</button>
	</div>

	{#if activeTab === 'color'}
		<div class="color-grid">
			{#each colors as color}
				<button
					class="color-btn"
					class:selected={selectedColor === color}
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
		style="position: fixed; bottom: 16px; right: 16px; width: auto"
	>
		{m.done()}
	</Button>
{/if}

<style>
	.hidden-input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.remove-avatar-btn {
		position: absolute;
		top: 8px;
		right: 8px;
		width: 40px;
		height: 40px;
		border-radius: 10px;
		background: white;
		color: #374151;
		border: none;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.2s;
	}

	.remove-avatar-btn:hover {
		background: #f3f4f6;
	}

	@media (prefers-color-scheme: dark) {
		.remove-avatar-btn {
			background: #4b5563;
			color: white;
		}
		.remove-avatar-btn:hover {
			background: #6b7280;
		}
	}

	.action-label {
		font-size: 14px;
		color: var(--k-text-color);
	}

	.avatar-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 12px;
		padding: 24px 16px;
		justify-items: center;
	}

	.default-avatar-btn {
		width: 72px;
		height: 72px;
		border-radius: 50%;
		background: #e5e7eb;
		border: none;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition:
			transform 0.2s,
			background 0.2s;
	}

	.default-avatar-btn:hover {
		transform: scale(1.05);
		background: #d1d5db;
	}

	.default-avatar-btn:active {
		transform: scale(0.95);
	}

	.nav-header {
		padding: 16px;
		padding-top: calc(16px + env(safe-area-inset-top));
	}

	.nav-btn {
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 8px;
		margin: -8px;
		color: var(--k-text-color);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.text-avatar-preview {
		width: 180px;
		height: 180px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		cursor: pointer;
	}

	.avatar-text {
		font-size: 56px;
		font-weight: 500;
		color: #831843;
	}

	.avatar-cursor {
		font-size: 56px;
		font-weight: 300;
		color: #831843;
		animation: blink 1s infinite;
		margin-left: -2px;
	}

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

	.color-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 16px;
		padding: 24px 24px;
		justify-items: center;
	}

	.color-btn {
		width: 72px;
		height: 72px;
		border-radius: 50%;
		border: 3px solid transparent;
		cursor: pointer;
		transition: transform 0.2s;
	}

	.color-btn.selected {
		border-color: #374151;
	}

	.color-btn:hover {
		transform: scale(1.05);
	}

	.color-btn:active {
		transform: scale(0.95);
	}
</style>
