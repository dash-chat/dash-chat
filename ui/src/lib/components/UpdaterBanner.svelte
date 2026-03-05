<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { onMount } from 'svelte';
	import { Progressbar } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isMobile, isTauriEnv } from '$lib/utils/environment';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose } from '@mdi/js';
	import type { Update } from '@tauri-apps/plugin-updater';

	type UpdateState = 'idle' | 'available' | 'downloading' | 'ready' | 'error';

	let updateState: UpdateState = $state('idle');
	let progress = $state(0);
	let contentLength = $state(0);
	let version = $state('');
	let update: Update | null = $state(null);

	// Set to 'available', 'downloading', 'ready', or 'error' to preview in dev mode
	const mockUpdate: false | 'available' | 'downloading' | 'ready' | 'error' = false;

	onMount(() => {
		if (!isTauriEnv() || isMobile || import.meta.env.DEV) {
			if (mockUpdate) simulateMockUpdate(mockUpdate);
			return;
		}

		checkForUpdate();
	});

	// Allow E2E tests and dev tools to trigger banner states
	$effect(() => {
		const handler = (event: CustomEvent<UpdateState>) => {
			version = '1.2.0';
			contentLength = 50_000_000;
			progress = contentLength;
			updateState = event.detail;
		};
		window.addEventListener('test-simulate-update', handler as EventListener);
		return () =>
			window.removeEventListener(
				'test-simulate-update',
				handler as EventListener,
			);
	});

	async function simulateMockUpdate(mode: 'available' | 'downloading' | 'ready' | 'error') {
		version = '1.2.0';
		if (mode === 'available') {
			updateState = 'available';
			return;
		}
		if (mode === 'error') {
			updateState = 'error';
			return;
		}
		if (mode === 'ready') {
			updateState = 'ready';
			return;
		}
		updateState = 'downloading';
		contentLength = 50_000_000;
		progress = 0;
		const chunk = 2_500_000;
		for (let i = 0; i < 20; i++) {
			await new Promise(r => setTimeout(r, 150));
			progress += chunk;
		}
		updateState = 'ready';
	}

	async function checkForUpdate() {
		try {
			try {
				const { check } = await import('@tauri-apps/plugin-updater');
				update = await check();
				if (!update) return;
			} catch (err) {
				console.warn('Update check failed:', err);
				return;
			}

			version = update.version;
			updateState = 'available';
		} catch (err) {
			console.warn('Update check failed:', err);
		}
	}

	async function downloadAndInstall() {
		if (!update) return;
		try {
			updateState = 'downloading';
			contentLength = 0;
			progress = 0;

			await update.downloadAndInstall(event => {
				if (event.event === 'Started') {
					contentLength = event.data.contentLength ?? 0;
				} else if (event.event === 'Progress') {
					progress += event.data.chunkLength;
				}
			});

			updateState = 'ready';
		} catch (err) {
			console.warn('Update download failed:', err);
			updateState = 'error';
		}
	}

	async function restart() {
		const { relaunch } = await import('@tauri-apps/plugin-process');
		await relaunch();
	}

	function dismiss() {
		updateState = 'idle';
	}

	function progressFraction(): number {
		if (contentLength <= 0) return 0;
		return Math.min(1, progress / contentLength);
	}
</script>

{#if updateState !== 'idle'}
	<div
		class="mx-4 mt-2 mb-1 flex items-center gap-3 rounded-lg bg-primary px-4 py-3 text-white dark:bg-primary"
		data-testid="updater-banner"
	>
		<div class="flex-1 min-w-0">
			{#if updateState === 'available'}
				<p class="text-sm font-medium" data-testid="updater-available">
					{m.updateAvailable()} — v{version}
				</p>
			{:else if updateState === 'downloading'}
				<p class="text-sm font-medium" data-testid="updater-downloading">
					{m.updateDownloading()}
				</p>
				<div class="mt-1.5">
					<Progressbar progress={progressFraction()} colors={{ trackBgIos: 'bg-white/30', trackBgMaterial: 'bg-white/30', activeBgIos: 'bg-white', activeBgMaterial: 'bg-white' }} />
				</div>
			{:else if updateState === 'ready'}
				<p class="text-sm font-medium" data-testid="updater-ready">
					{m.updateReady()}
				</p>
			{:else if updateState === 'error'}
				<p class="text-sm font-medium" data-testid="updater-error">
					{m.updateError()}
				</p>
			{/if}
		</div>

		<div class="flex items-center gap-1 shrink-0">
			{#if updateState === 'available'}
				<button
					class="rounded-md bg-white/20 px-3 py-1 text-sm font-semibold text-white hover:bg-white/30 active:bg-white/40"
					onclick={downloadAndInstall}
					data-testid="updater-download-btn"
				>
					{m.updateDownloadAction()}
				</button>
			{:else if updateState === 'ready'}
				<button
					class="rounded-md bg-white/20 px-3 py-1 text-sm font-semibold text-white hover:bg-white/30 active:bg-white/40"
					onclick={restart}
					data-testid="updater-restart-btn"
				>
					{m.updateRestart()}
				</button>
			{:else if updateState === 'error'}
				<button
					class="rounded-md bg-white/20 px-3 py-1 text-sm font-semibold text-white hover:bg-white/30 active:bg-white/40"
					onclick={checkForUpdate}
					data-testid="updater-retry-btn"
				>
					{m.updateRetry()}
				</button>
			{/if}

			{#if updateState !== 'downloading'}
				<button
					class="rounded-md p-1 text-white/70 hover:text-white hover:bg-white/10"
					onclick={dismiss}
					data-testid="updater-dismiss-btn"
					aria-label="Dismiss"
				>
					<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 18px"></wa-icon>
				</button>
			{/if}
		</div>
	</div>
{/if}
