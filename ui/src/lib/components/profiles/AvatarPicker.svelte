<script lang="ts">
	import { flushSync } from 'svelte';
	import TextAvatarPicker from './TextAvatarPicker.svelte';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiCamera, mdiImage } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Link, Navbar } from 'konsta/svelte';
	import { resizeAndExport } from '$lib/utils/image';
	import { isMobile, isIos } from '$lib/utils/environment';
	import Avatar from './Avatar.svelte';
	import { editorPrefill, joinName } from './avatar-helpers';

	let {
		avatar = $bindable(),
		name,
		surname,
		colorSeed,
		inModalState = $bindable(false),
		onSelect,
		onClose,
		onSave,
		saveLabel,
		saveDisabled = false,
	}: {
		avatar?: string | undefined;
		name?: string | undefined;
		surname?: string | undefined;
		colorSeed?: string | undefined;
		inModalState?: boolean;
		onSelect?: () => void;
		onClose?: () => void;
		onSave?: () => void;
		saveLabel?: string;
		saveDisabled?: boolean;
	} = $props();

	const displayName = $derived(joinName(name, surname));

	let view = $state<'picker' | 'text'>('picker');
	$effect(() => {
		inModalState = view === 'text';
	});
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

		// Same emoji-to-circle ratio as the picker grid (36px in a 56px bubble),
		// so the avatar keeps its proportions once rendered from this image.
		ctx.font =
			'165px "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText(emoji, 128, 128);

		avatar = canvas.toDataURL('image/png');
		onSelect?.();
	}

	let focusTextAvatarInput: (() => void) | undefined = $state();

	function openTextEditor() {
		// flushSync mounts TextAvatarPicker synchronously inside this click
		// handler, so iOS still considers us in user-gesture context when we
		// call focus — that's the only way the soft keyboard opens on a
		// programmatic focus on iOS WKWebView.
		flushSync(() => {
			view = 'text';
		});
		focusTextAvatarInput?.();
	}
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
					<Link onClick={onSave} data-testid="edit-photo-save-btn">
						{saveLabel || m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>
	{/if}

	<!-- Avatar preview with remove button -->
	<div class="column" style="align-items: center; padding: 16px 0 24px;">
		<div style="position: relative; display: inline-block;">
			<Avatar
				style="--size: 80px"
				image={avatar}
				name={displayName}
				{colorSeed}
			/>
			{#if avatar}
				<button
					class="absolute -top-1 -end-1 w-6 h-6 rounded-full bg-white text-gray-700 border-none cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-gray-100 dark:bg-gray-600 dark:text-white dark:hover:bg-gray-500"
					onclick={removeAvatar}
					aria-label={m.removePhoto()}
				>
					<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 14px"
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
				data-testid="edit-photo-text"
			>
				Aa
			</Button>
			<span class="text-sm" style="color: var(--k-text-color)">{m.text()}</span>
		</div>
	</div>

	<!-- Divider -->
	<div style="height: 1px; background: var(--k-hairline-color);"></div>

	<!-- Default avatars grid. Signal Desktop's visual rhythm: its 56px bubbles
	     hold a 48px avatar, so circles sit ~17px apart; our artwork fills the
	     bubble, so the gap carries all of that spacing. -->
	<div class="flex flex-wrap justify-center gap-4 py-6 px-4">
		{#each defaultAvatars as emoji}
			<button
				class="w-14 h-14 rounded-full bg-gray-200 border-none cursor-pointer flex items-center justify-center transition-all duration-200 hover:scale-105 hover:bg-gray-300 active:scale-95"
				onclick={() => selectDefaultAvatar(emoji)}
			>
				<span style="font-size: 36px">{emoji}</span>
			</button>
		{/each}
	</div>
{:else}
	<TextAvatarPicker
		existingAvatar={avatar}
		prefill={editorPrefill(name ?? '', surname, colorSeed)}
		bind:focusInput={focusTextAvatarInput}
		onSelect={nextAvatar => {
			avatar = nextAvatar;
			view = 'picker';
		}}
		onClose={() => (view = 'picker')}
	/>
{/if}
