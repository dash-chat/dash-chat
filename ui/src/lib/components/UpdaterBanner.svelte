<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { onMount } from 'svelte';
	import { Progressbar } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isMobile, isTauriEnv } from '$lib/utils/environment';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiUpdate, mdiClose } from '@mdi/js';
	import type { Update } from '@tauri-apps/plugin-updater';

	type UpdateState = 'idle' | 'available' | 'downloading' | 'ready' | 'error';

	let updateState: UpdateState = $state('idle');
	let progress = $state(0);
	let contentLength = $state(0);
	let version = $state('');
	let pendingUpdate: Update | null = $state(null);

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
		if (mode === 'error') {
			updateState = 'error';
			return;
		}
		if (mode === 'available') {
			updateState = 'available';
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

	async function startDownload() {
		if (!pendingUpdate) return;

		updateState = 'downloading';
		contentLength = 0;
		progress = 0;

		try {
			await pendingUpdate.downloadAndInstall(event => {
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

	function onBannerClick() {
		if (updateState === 'available') {
			startDownload();
		} else if (updateState === 'ready') {
			restart();
		} else if (updateState === 'error') {
			checkForUpdate();
		}
	}

	function progressFraction(): number {
		if (contentLength <= 0) return 0;
		return Math.min(1, progress / contentLength);
	}
</script>

{#if updateState !== 'idle'}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_tabindex -->
	<div
		class="flex w-full items-center gap-3 bg-[#3478F6] px-4 py-3 text-white cursor-pointer"
		data-testid="updater-banner"
		onclick={updateState !== 'downloading' ? onBannerClick : undefined}
		role={updateState !== 'downloading' ? 'button' : undefined}
		tabindex={updateState !== 'downloading' ? 0 : undefined}
	>
		<wa-icon
			src={wrapPathInSvg(mdiUpdate)}
			class={updateState === 'downloading' ? 'animate-spin' : ''}
			style="font-size: 24px; color: white; flex-shrink: 0"
		></wa-icon>

		<div class="flex flex-1 flex-col items-start gap-1 text-start">
			{#if updateState === 'available'}
				<span class="text-sm font-semibold" data-testid="updater-available">
					{m.updateAvailable()} — v{version}
				</span>
				<span class="text-xs opacity-80">{m.updateTapToUpdate()}</span>
			{:else if updateState === 'downloading'}
				<span class="text-sm font-semibold" data-testid="updater-downloading">
					{m.updateDownloading()}
				</span>
				<div class="w-full">
					<Progressbar progress={progressFraction()} class="updater-progress" />
				</div>
			{:else if updateState === 'ready'}
				<span class="text-sm font-semibold" data-testid="updater-ready">
					{m.updateReady()}
				</span>
				<span class="text-xs opacity-80">{m.updateTapToRestart()}</span>
			{:else if updateState === 'error'}
				<span class="text-sm font-semibold" data-testid="updater-error">
					{m.updateError()}
				</span>
			{/if}
		</div>

		<button
			class="flex-shrink-0 p-1"
			data-testid="updater-dismiss-btn"
			onclick={(e: MouseEvent) => { e.stopPropagation(); dismiss(); }}
			type="button"
			aria-label="Dismiss"
		>
			<wa-icon
				src={wrapPathInSvg(mdiClose)}
				style="font-size: 20px; color: white"
			></wa-icon>
		</button>
	</div>
{/if}

<style>
	:global(.updater-progress) {
		--k-color-brand-primary: white;
		height: 4px;
		border-radius: 2px;
		background-color: rgba(255, 255, 255, 0.3);
	}
</style>
