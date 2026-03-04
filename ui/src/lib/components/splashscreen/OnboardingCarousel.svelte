<script lang="ts">
	import { Page, Button, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { splashscreenDismissed } from './utils';
	import { isMobile } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiHandWaveOutline,
		mdiDatabaseLockOutline,
		mdiShieldLockOutline,
		mdiWifiOff,
		mdiFlaskOutline,
		mdiRocketLaunchOutline,
	} from '@mdi/js';

	interface CarouselPage {
		icon: string;
		title: () => string;
		description: () => string;
	}

	const pages: CarouselPage[] = [
		{
			icon: mdiHandWaveOutline,
			title: () => m.welcomeToDashChat(),
			description: () => m.aPrivateP2pChatApp(),
		},
		{
			icon: mdiDatabaseLockOutline,
			title: () => m.youOwnYourData(),
			description: () => m.messagesStoredOnDevice(),
		},
		{
			icon: mdiShieldLockOutline,
			title: () => m.preserveYourPrivacy(),
			description: () => m.allMessagesEncrypted(),
		},
		{
			icon: mdiWifiOff,
			title: () => m.chatEvenWhenOffline(),
			description: () => m.chatOfflineExplanation(),
		},
		{
			icon: mdiFlaskOutline,
			title: () => m.dashChatIsPreAlpha(),
			description: () => m.preAlphaExplanation(),
		},
		{
			icon: mdiRocketLaunchOutline,
			title: () => m.thatsIt(),
			description: () => m.haveFunUsingDashChat(),
		},
	];

	let currentPage = $state(0);
	const isLastPage = $derived(currentPage === pages.length - 1);
	const theme = $derived(useTheme());

	function next() {
		if (currentPage < pages.length - 1) {
			currentPage++;
		}
	}

	function back() {
		if (currentPage > 0) {
			currentPage--;
		}
	}

	async function startApp() {
		if (isMobile) {
			try {
				const {
					isPermissionGranted,
					requestPermission,
				} = await import('@tauri-apps/plugin-notification');

				const granted = await isPermissionGranted();
				if (!granted) {
					await requestPermission();
				}
			} catch (e) {
				console.error('Failed to setup push notifications:', e);
			}
		}
		splashscreenDismissed.dismiss();
	}
</script>

<Page>
	<div class="flex flex-col items-center justify-center px-6" style="height: 100%">
		<div
			class="flex flex-col items-center justify-center gap-6 w-full max-w-md"
			class:bg-white={isWideScreen.value && theme === 'material'}
			class:dark:bg-neutral-800={isWideScreen.value && theme === 'material'}
			class:rounded-2xl={isWideScreen.value}
			class:shadow-lg={isWideScreen.value && theme === 'material'}
			class:p-10={isWideScreen.value}
		>
			{#each pages as page, i}
				{#if i === currentPage}
					<div class="flex flex-col items-center text-center gap-4 min-h-[220px] justify-center">
						<div
							class="w-20 h-20 rounded-full flex items-center justify-center"
							class:bg-primary={theme === 'material'}
							class:bg-blue-500={theme === 'ios'}
						>
							<img
								src={wrapPathInSvg(page.icon)}
								alt=""
								class="w-10 h-10 invert"
							/>
						</div>

						<h2 class="text-2xl font-bold">{page.title()}</h2>
						<p class="text-base opacity-70 max-w-xs">{page.description()}</p>
					</div>
				{/if}
			{/each}

			<!-- Dot indicators -->
			<div class="flex gap-2">
				{#each pages as _, i}
					<button
						type="button"
						class="w-2.5 h-2.5 rounded-full transition-colors border-0 p-0 cursor-pointer"
						class:bg-primary={i === currentPage && theme === 'material'}
						class:bg-blue-500={i === currentPage && theme === 'ios'}
						class:opacity-100={i === currentPage}
						class:bg-gray-300={i !== currentPage}
						class:dark:bg-gray-600={i !== currentPage}
						class:opacity-60={i !== currentPage}
						onclick={() => (currentPage = i)}
						aria-label="Page {i + 1}"
					></button>
				{/each}
			</div>

			</div>

		<!-- Navigation buttons -->
		<div
			class="fixed left-6 right-6 flex justify-between items-center pointer-events-none"
			style="bottom: env(safe-area-inset-bottom, 0px)"
		>
			<div class="pointer-events-auto">
				{#if currentPage > 0}
					<Button
						outline
						rounded
						onClick={back}
						data-testid="onboarding-back-btn"
					>
						{m.back()}
					</Button>
				{/if}
			</div>

			<div class="pointer-events-auto">
				{#if isLastPage}
					<Button
						rounded
						onClick={startApp}
						data-testid="onboarding-start-btn"
					>
						{m.startApp()}
					</Button>
				{:else}
					<Button
						rounded
						onClick={next}
						data-testid="onboarding-next-btn"
					>
						{m.next()}
					</Button>
				{/if}
			</div>
		</div>
	</div>
</Page>
