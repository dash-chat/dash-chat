<script lang="ts">
	import { onMount } from 'svelte';
	import { Progressbar } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isMobile, isTauriEnv } from '$lib/utils/environment';
	import type { Update } from '@tauri-apps/plugin-updater';

	type UpdateState = 'idle' | 'available' | 'downloading' | 'ready' | 'error';

	let updateState: UpdateState = $state('idle');
	let progress = $state(0);
	let contentLength = $state(0);
	let version = $state('');
	let pendingUpdate: Update | null = $state(null);

	// Set to 'available', 'downloading', 'ready', or 'error' to preview in dev mode
	const mockUpdate: false | UpdateState = false;

	onMount(() => {
		if (!isTauriEnv() || isMobile || import.meta.env.DEV) {
			if (mockUpdate && mockUpdate !== 'idle') simulateMockUpdate(mockUpdate);
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

	async function simulateMockUpdate(mode: UpdateState) {
		version = '1.2.0';
		if (mode === 'available') {
			updateState = 'available';
			return;
		}
		if (mode === 'error') {
			updateState = 'error';
			return;
		}
		if (mode === 'downloading') {
			updateState = 'downloading';
			contentLength = 50_000_000;
			progress = 0;
			const chunk = 2_500_000;
			for (let i = 0; i < 20; i++) {
				await new Promise((r) => setTimeout(r, 150));
				progress += chunk;
			}
			updateState = 'ready';
			return;
		}
		if (mode === 'ready') {
			updateState = 'ready';
			contentLength = 50_000_000;
			progress = contentLength;
		}
	}

	async function checkForUpdate() {
		try {
			let update: Update | null;
			try {
				const { check } = await import('@tauri-apps/plugin-updater');
				update = await check();
				if (!update) return;
			} catch (err) {
				console.warn('Update check failed:', err);
				return;
			}

			version = update.version;
			pendingUpdate = update;
			updateState = 'available';
		} catch (err) {
			console.warn('Update check failed:', err);
		}
	}

	async function downloadAndInstall() {
		if (!pendingUpdate) return;

		try {
			updateState = 'downloading';
			contentLength = 0;
			progress = 0;

			await pendingUpdate.downloadAndInstall((event) => {
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
		class="flex items-center gap-2 px-4 py-2 text-white text-sm"
		style="background-color: var(--color-brand-primary)"
		data-testid="updater-banner"
	>
		{#if updateState === 'available'}
			<span class="flex-1" data-testid="updater-available">
				{m.updateAvailable()} — v{version}
			</span>
			<button
				class="font-semibold underline whitespace-nowrap"
				onclick={downloadAndInstall}
				data-testid="updater-download-btn"
			>
				{m.updateDownload()}
			</button>
			<button
				class="opacity-70 ml-1"
				onclick={dismiss}
				data-testid="updater-dismiss-btn"
				aria-label="Dismiss"
			>
				✕
			</button>
		{:else if updateState === 'downloading'}
			<div class="flex-1 min-w-0" data-testid="updater-downloading">
				<span>{m.updateDownloading()}</span>
				<div class="mt-1">
					<Progressbar progress={progressFraction()} />
				</div>
			</div>
			<span class="text-xs opacity-80 whitespace-nowrap">
				{Math.round(progressFraction() * 100)}%
			</span>
		{:else if updateState === 'ready'}
			<span class="flex-1" data-testid="updater-ready">
				{m.updateReady()}
			</span>
			<button
				class="font-semibold underline whitespace-nowrap"
				onclick={restart}
				data-testid="updater-restart-btn"
			>
				{m.updateRestart()}
			</button>
			<button
				class="opacity-70 ml-1"
				onclick={dismiss}
				data-testid="updater-later-btn"
				aria-label="Dismiss"
			>
				✕
			</button>
		{:else if updateState === 'error'}
			<span class="flex-1" data-testid="updater-error">
				{m.updateError()}
			</span>
			<button
				class="font-semibold underline whitespace-nowrap"
				onclick={() => { updateState = 'available'; }}
				data-testid="updater-retry-btn"
			>
				{m.updateRetry()}
			</button>
			<button
				class="opacity-70 ml-1"
				onclick={dismiss}
				data-testid="updater-ok-btn"
				aria-label="Dismiss"
			>
				✕
			</button>
		{/if}
	</div>
{/if}
