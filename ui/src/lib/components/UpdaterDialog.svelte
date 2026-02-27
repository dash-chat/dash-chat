<script lang="ts">
	import { onMount } from 'svelte';
	import { Dialog, DialogButton, Progressbar } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isTauriEnv } from '$lib/utils/environment';

	type UpdateState = 'idle' | 'downloading' | 'ready' | 'error';

	let updateState: UpdateState = $state('idle');
	let progress = $state(0);
	let contentLength = $state(0);
	let version = $state('');

	// Set to 'download' or 'error' to preview the dialog in dev mode
	const mockUpdate: false | 'download' | 'error' = false;

	onMount(() => {
		// The updater plugin is only loaded in production desktop builds
		// (see src-tauri/src/lib.rs:41-66)
		if (!isTauriEnv() || import.meta.env.DEV) {
			if (mockUpdate) simulateMockUpdate(mockUpdate);
			return;
		}

		checkForUpdate();
	});

	// Allow E2E tests and dev tools to trigger dialog states
	$effect(() => {
		const handler = (event: CustomEvent<UpdateState>) => {
			version = '1.2.0';
			contentLength = 50_000_000;
			progress = contentLength;
			updateState = event.detail;
		};
		window.addEventListener('test-simulate-update', handler as EventListener);
		return () => window.removeEventListener('test-simulate-update', handler as EventListener);
	});

	async function simulateMockUpdate(mode: 'download' | 'error') {
		version = '1.2.0';
		if (mode === 'error') {
			updateState = 'error';
			return;
		}
		updateState = 'downloading';
		contentLength = 50_000_000;
		progress = 0;
		const chunk = 2_500_000;
		for (let i = 0; i < 20; i++) {
			await new Promise((r) => setTimeout(r, 150));
			progress += chunk;
		}
		updateState = 'ready';
	}

	async function checkForUpdate() {
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const update = await check();
			if (!update) return;

			version = update.version;
			updateState = 'downloading';
			contentLength = 0;
			progress = 0;

			await update.downloadAndInstall((event) => {
				if (event.event === 'Started') {
					contentLength = event.data.contentLength ?? 0;
				} else if (event.event === 'Progress') {
					progress += event.data.chunkLength;
				}
			});

			updateState = 'ready';
		} catch (err) {
			console.error('Update check failed:', err);
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

<Dialog opened={updateState === 'downloading'} data-testid="updater-downloading">
	{#snippet title()}
		{m.updateAvailable()} — v{version}
	{/snippet}
	<p class="px-4 text-sm opacity-70">{m.updateDownloading()}</p>
	<div class="mx-4 mt-3 mb-1">
		<Progressbar progress={progressFraction()} />
	</div>
	<p class="px-4 text-xs opacity-50 text-right">{Math.round(progressFraction() * 100)}%</p>
</Dialog>

<Dialog opened={updateState === 'ready'} data-testid="updater-ready">
	{#snippet title()}
		{m.updateAvailable()} — v{version}
	{/snippet}
	<p class="px-4 text-sm opacity-70">{m.updateReady()}</p>
	{#snippet buttons()}
		<DialogButton onClick={dismiss} data-testid="updater-later-btn">
			{m.updateLater()}
		</DialogButton>
		<DialogButton strong onClick={restart} data-testid="updater-restart-btn">
			{m.updateRestart()}
		</DialogButton>
	{/snippet}
</Dialog>

<Dialog opened={updateState === 'error'} onBackdropClick={dismiss} data-testid="updater-error">
	{#snippet title()}
		{m.updateAvailable()}
	{/snippet}
	<p class="px-4 text-sm opacity-70">{m.updateError()}</p>
	{#snippet buttons()}
		<DialogButton onClick={dismiss} data-testid="updater-ok-btn">{m.updateOk()}</DialogButton>
	{/snippet}
</Dialog>
