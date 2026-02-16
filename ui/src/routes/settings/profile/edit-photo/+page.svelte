<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import type { Error } from 'dash-chat-stores';
	import { getContext, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiCamera, mdiImage } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page, Preloader, useTheme } from 'konsta/svelte';
	import { resizeAndExport } from '$lib/utils/image';
	import { showToast } from '$lib/utils/toasts';
	import { isMobile } from '$lib/utils/environment';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let avatar = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	let originalAvatar = $state<string | undefined>(undefined);

	myProfile.subscribe(m => {
		m.then(myProfile => {
			if (!name) name = myProfile?.name || '';
			if (originalAvatar === undefined) {
				originalAvatar = myProfile?.avatar;
			}
			if (avatar === undefined) {
				avatar = myProfile?.avatar;
			}
			if (!surname) surname = myProfile?.surname;
			if (!about) about = myProfile?.about;
		});
	});

	// Handle incoming text avatar from the text page
	onMount(() => {
		const textParam = $page.url.searchParams.get('text');
		const colorParam = $page.url.searchParams.get('color');
		if (textParam && colorParam) {
			generateTextAvatar(textParam, colorParam);
			// Clear the URL params
			goto('/settings/profile/edit-photo', { replaceState: true });
		}
	});

	function generateTextAvatar(text: string, bgColor: string) {
		const canvas = document.createElement('canvas');
		canvas.width = 256;
		canvas.height = 256;
		const ctx = canvas.getContext('2d')!;

		// Fill with background color
		ctx.fillStyle = bgColor;
		ctx.beginPath();
		ctx.arc(128, 128, 128, 0, Math.PI * 2);
		ctx.fill();

		// Draw the text
		ctx.fillStyle = '#831843';
		ctx.font = '500 100px sans-serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText(text.toUpperCase(), 128, 135);

		avatar = canvas.toDataURL('image/png');
	}

	// Default emoji avatars (4x4 grid)
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

	async function save() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about,
			});
			goto('/settings/profile');
		} catch (e) {
			console.error(e);
			const error = e as Error;
			switch (error.kind) {
				case 'AuthorOperation':
					showToast(m.errorSetProfile(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'error');
			}
		}
	}

	const theme = $derived(useTheme());
	const hasChanges = $derived(avatar !== originalAvatar);

	let avatarFilePicker: HTMLInputElement;
	function onAvatarUploaded() {
		if (avatarFilePicker.files && avatarFilePicker.files[0]) {
			const reader = new FileReader();
			reader.onload = e => {
				const img = new Image();
				img.crossOrigin = 'anonymous';
				img.onload = () => {
					avatar = resizeAndExport(img);
					avatarFilePicker.value = '';
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
		// Create a canvas to render the emoji as an image
		const canvas = document.createElement('canvas');
		canvas.width = 256;
		canvas.height = 256;
		const ctx = canvas.getContext('2d')!;

		// Fill with a light background color
		ctx.fillStyle = '#e5e7eb';
		ctx.beginPath();
		ctx.arc(128, 128, 128, 0, Math.PI * 2);
		ctx.fill();

		// Draw the emoji with proper emoji font stack
		ctx.font =
			'140px "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText(emoji, 128, 128);

		avatar = canvas.toDataURL('image/png');
	}
</script>

<input
	type="file"
	accept="image/*"
	bind:this={avatarFilePicker}
	style="display: none"
	onchange={onAvatarUploaded}
/>

<Page>
	{#await $myProfile}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then myProfile}
		<!-- Close button -->
		<div class="p-4">
			<button
				class="close-btn"
				onclick={() => goto('/settings/profile')}
				aria-label="Close"
			>
				<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 28px"
				></wa-icon>
			</button>
		</div>

		<div class="column" style="flex: 1; overflow-y: auto;">
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
							<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 20px"
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
							<wa-icon src={wrapPathInSvg(mdiCamera)} style="font-size: 28px"
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
						<wa-icon src={wrapPathInSvg(mdiImage)} style="font-size: 28px"
						></wa-icon>
					</Button>
					<span class="action-label">{m.photo()}</span>
				</div>

				<div class="column" style="align-items: center; gap: 8px;">
					<Button
						tonal
						onClick={() => goto('/settings/profile/edit-photo/text')}
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
		</div>

		<!-- Save button -->
		<Button
			rounded
			tonal
			disabled={!hasChanges}
			onClick={save}
			style="position: fixed; bottom: 16px; right: 16px; width: auto"
		>
			{m.save()}
		</Button>
	{/await}
</Page>

<style>
	.close-btn {
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
		padding: 24px 16px 80px;
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
</style>
