<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { objectUrl } from '$lib/actions/object-url';
	import PermissionSettingsSheet from '$lib/components/PermissionSettingsSheet.svelte';
	import {
		type RecentPhoto,
		type RecentPhotosPermission,
		RECENT_PHOTOS_LIMIT,
		cachedRecentPhotos,
		getRecentPhotosPermission,
		listRecentPhotos,
		loadRecentPhotoFile,
		recentPhotosSupported,
		requestRecentPhotosPermission,
	} from '$lib/utils/recent-photos';

	interface Props {
		onFiles: (files: File[]) => void;
	}

	let { onFiles }: Props = $props();

	let permission = $state<RecentPhotosPermission | undefined>(undefined);
	let photos = $state<RecentPhoto[]>([]);
	let loadingId = $state<string | undefined>(undefined);
	let showSettingsSheet = $state(false);

	onMount(init);

	async function init() {
		// Render preloaded photos instantly when the cache is already warm.
		const cached = cachedRecentPhotos();
		if (cached) {
			photos = cached.filter(p => p.thumbnail);
			permission = 'granted';
			return;
		}
		if (!recentPhotosSupported) return;
		permission = await getRecentPhotosPermission();
		if (permission === 'granted') await load();
	}

	async function load() {
		try {
			photos = (await listRecentPhotos(RECENT_PHOTOS_LIMIT)).filter(
				p => p.thumbnail,
			);
		} catch (e) {
			console.error('Failed to list recent photos', e);
			photos = [];
		}
	}

	async function allow() {
		permission = await requestRecentPhotosPermission();
		if (permission === 'granted') await load();
		// The OS dialog won't show once the permission is permanently denied, so
		// guide the user to grant it from the app's settings instead.
		else showSettingsSheet = true;
	}

	async function add(photo: RecentPhoto) {
		loadingId = photo.id;
		try {
			const file = await loadRecentPhotoFile(photo);
			onFiles([file]);
		} catch (e) {
			console.error('Failed to load photo', e);
		} finally {
			loadingId = undefined;
		}
	}
</script>

{#if photos.length > 0}
	<div
		class="flex gap-2 overflow-x-auto px-2 pb-4"
		data-testid="message-input-recent-photos"
	>
		{#each photos as photo, i (photo.id)}
			<button
				type="button"
				class="recent-tile relative h-[100px] w-[100px] shrink-0 overflow-hidden"
				data-testid="message-input-recent-photo-{i}"
				onclick={() => add(photo)}
			>
				<img
					use:objectUrl={photo.thumbnail}
					alt={photo.name}
					class="block h-full w-full object-cover"
				/>
				{#if loadingId === photo.id}
					<span class="tile-spinner absolute inset-0"></span>
				{/if}
			</button>
		{/each}
	</div>
{:else if permission === 'prompt' || permission === 'denied'}
	<div class="flex flex-col items-center gap-4 px-5 pt-2 pb-4 text-center">
		<span class="text-sm" style="color: var(--k-text-color)">
			{m.recentPhotosPermissionPrompt()}
		</span>
		<Button
			rounded
			tonal
			onClick={allow}
			data-testid="message-input-recent-photos-allow"
			style="width: auto"
		>
			{m.recentPhotosAllowAccess()}
		</Button>
	</div>
{/if}

<PermissionSettingsSheet
	bind:opened={showSettingsSheet}
	title={m.recentPhotosSettingsTitle()}
	subtitle={m.recentPhotosSettingsSubtitle()}
	steps={[
		m.recentPhotosSettingsStep1(),
		m.recentPhotosSettingsStep2(),
		m.recentPhotosSettingsStep3(),
	]}
/>

<style>
	.recent-tile {
		border-radius: 8px;
		background: rgba(128, 128, 128, 0.1);
		cursor: pointer;
		border: none;
		padding: 0;
	}

	.tile-spinner {
		background: rgba(0, 0, 0, 0.25);
	}
	.tile-spinner::after {
		content: '';
		position: absolute;
		inset-inline-start: 50%;
		top: 50%;
		width: 22px;
		height: 22px;
		margin-inline-start: -11px;
		margin-top: -11px;
		border: 2px solid rgba(255, 255, 255, 0.5);
		border-top-color: white;
		border-radius: 50%;
		animation: tile-spin 0.7s linear infinite;
	}
	@keyframes tile-spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
