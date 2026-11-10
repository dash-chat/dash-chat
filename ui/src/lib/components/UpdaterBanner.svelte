<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { onMount } from 'svelte';
	import { Progressbar } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isMobile, isTauriEnv } from '$lib/utils/environment';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiRefresh, mdiClose } from '@mdi/js';
	import type { Update } from '@tauri-apps/plugin-updater';

	type BannerState =
		| 'hidden'
		| 'available'
		| 'misplaced'
		| 'moving'
		| 'moveError'
		| 'downloading'
		| 'ready'
		| 'error';
	type MockState = Exclude<BannerState, 'hidden'>;

	let bannerState: BannerState = $state('hidden');
	let progress = $state(0);
	let contentLength = $state(0);
	let version = $state('');
	let update: Update | null = $state(null);

	// Set to a state name to preview the banner in dev mode
	const mockUpdate: false | MockState = false;

	onMount(() => {
		if (!isTauriEnv() || isMobile || import.meta.env.DEV) {
			if (mockUpdate) simulateMockUpdate(mockUpdate);
			return;
		}

		checkForUpdate();
	});

	// Allow E2E tests and dev tools to trigger banner states
	$effect(() => {
		const handler = (event: CustomEvent<BannerState>) => {
			version = '1.2.0';
			contentLength = 50_000_000;
			progress = contentLength;
			bannerState = event.detail;
		};
		window.addEventListener('test-simulate-update', handler as EventListener);
		return () =>
			window.removeEventListener(
				'test-simulate-update',
				handler as EventListener,
			);
	});

	async function simulateMockUpdate(mode: MockState) {
		version = '1.2.0';
		if (mode !== 'downloading' && mode !== 'ready') {
			bannerState = mode;
			return;
		}
		bannerState = 'downloading';
		contentLength = 50_000_000;
		progress = 0;
		const chunk = 2_500_000;
		for (let i = 0; i < 20; i++) {
			await new Promise(r => setTimeout(r, 150));
			progress += chunk;
		}
		bannerState = 'ready';
	}

	async function checkForUpdate() {
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			update = await check();
			if (!update) {
				bannerState = 'hidden';
				return;
			}
			version = update.version;
			bannerState = (await isMisplaced()) ? 'misplaced' : 'available';
		} catch (err) {
			console.warn('Update check failed:', err);
		}
	}

	/** Whether the app runs from somewhere it cannot replace itself. */
	async function isMisplaced() {
		try {
			const { needsMove } = await import(
				'tauri-plugin-move-to-applications-api'
			);
			return await needsMove();
		} catch (err) {
			console.warn('Checking the app location failed:', err);
			return false;
		}
	}

	/** Moves the app to Applications, where it can update itself, and
	 * relaunches it from there unless the user cancels. */
	async function moveToApplications() {
		bannerState = 'moving';
		try {
			const { moveToApplicationsFolder } = await import(
				'tauri-plugin-move-to-applications-api'
			);
			// Returns only if the user cancels the administrator prompt.
			await moveToApplicationsFolder();
			settle('moving', 'misplaced');
		} catch (err) {
			console.warn('Moving to Applications failed:', err);
			settle('moving', 'moveError');
		}
	}

	/** Ends the operation `from` in state `to`, unless the banner was
	 * dismissed while it ran. */
	function settle(from: BannerState, to: BannerState) {
		if (bannerState === from) bannerState = to;
	}

	async function downloadAndInstall() {
		if (!update) return;

		try {
			bannerState = 'downloading';
			contentLength = 0;
			progress = 0;

			await update.downloadAndInstall(event => {
				if (event.event === 'Started') {
					contentLength = event.data.contentLength ?? 0;
				} else if (event.event === 'Progress') {
					progress += event.data.chunkLength;
				}
			});

			settle('downloading', 'ready');
		} catch (err) {
			console.warn('Update download failed:', err);
			settle('downloading', 'error');
		}
	}

	async function restart() {
		const { relaunch } = await import('@tauri-apps/plugin-process');
		await relaunch();
	}

	function dismiss() {
		bannerState = 'hidden';
	}

	function progressFraction(): number {
		if (contentLength <= 0) return 0;
		return Math.min(1, progress / contentLength);
	}

	function handleBannerClick() {
		if (bannerState === 'available') {
			downloadAndInstall();
		} else if (bannerState === 'misplaced' || bannerState === 'moveError') {
			moveToApplications();
		} else if (bannerState === 'ready') {
			restart();
		} else if (bannerState === 'error') {
			checkForUpdate();
		}
	}
</script>

{#if bannerState !== 'hidden'}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="flex items-center gap-3 bg-brand-primary px-4 py-3 text-white cursor-pointer"
		onclick={handleBannerClick}
		data-testid="updater-banner"
	>
		<wa-icon
			src={wrapPathInSvg(mdiRefresh)}
			class={bannerState === 'downloading' || bannerState === 'moving'
				? 'animate-spin'
				: ''}
			style="font-size: 24px; color: white;"
		></wa-icon>

		<div class="flex-1 min-w-0">
			{#if bannerState === 'available' || bannerState === 'misplaced'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateAvailable()}
				</div>
				<div class="text-xs opacity-80">
					{bannerState === 'available'
						? m.updateTapToUpdate()
						: m.updateTapToMove()}
				</div>
			{:else if bannerState === 'moving'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateMoving()}
				</div>
			{:else if bannerState === 'moveError'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateMoveError()}
				</div>
				<div class="text-xs opacity-80">{m.updateTapToRetry()}</div>
			{:else if bannerState === 'downloading'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateDownloading()}
				</div>
				<div class="mt-1">
					<Progressbar
						progress={progressFraction()}
						colors={{
							trackBgIos: 'bg-white/30',
							trackBgMaterial: 'bg-white/30',
							activeBgIos: 'bg-white',
							activeBgMaterial: 'bg-white',
						}}
					/>
				</div>
			{:else if bannerState === 'ready'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateReady()}
				</div>
				<div class="text-xs opacity-80">{m.updateTapToRestart()}</div>
			{:else if bannerState === 'error'}
				<div class="text-sm font-semibold" data-testid="updater-banner-title">
					{m.updateError()}
				</div>
				<div class="text-xs opacity-80">{m.updateTapToRetry()}</div>
			{/if}
		</div>

		<button
			class="shrink-0 p-1 rounded-full hover:bg-white/20"
			onclick={e => {
				e.stopPropagation();
				dismiss();
			}}
			data-testid="updater-dismiss-btn"
			aria-label="Dismiss"
		>
			<wa-icon
				src={wrapPathInSvg(mdiClose)}
				style="font-size: 20px; color: white;"
			></wa-icon>
		</button>
	</div>
{/if}
