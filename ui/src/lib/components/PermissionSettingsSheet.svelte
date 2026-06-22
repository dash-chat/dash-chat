<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Sheet, Block, Button } from 'konsta/svelte';
	import { openAppSettings } from '@tauri-apps/plugin-barcode-scanner';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiRadioboxMarked } from '@mdi/js';

	interface Props {
		opened: boolean;
		title: string;
		subtitle: string;
		steps: string[];
	}

	let { opened = $bindable(false), title, subtitle, steps }: Props = $props();

	// The Settings button always routes to this app's OS settings page, the only
	// place to grant a permanently-denied permission. (barcode-scanner ships the
	// generic cross-platform helper.)
	async function settings() {
		opened = false;
		await openAppSettings();
	}
</script>

<Sheet class="pb-safe" {opened} onBackdropClick={() => (opened = false)}>
	<div class="flex flex-col items-center">
		<div class="sheet-handle"></div>
	</div>
	<Block>
		<div
			class="flex flex-col gap-5 pb-2"
			data-testid="permission-settings-sheet"
		>
			<div class="text-center">
				<h2 class="text-xl font-semibold" style="color: var(--k-text-color)">
					{title}
				</h2>
				<p class="mt-1 text-sm opacity-70" style="color: var(--k-text-color)">
					{subtitle}
				</p>
			</div>
			<ol
				class="flex flex-col gap-3 text-start text-sm"
				style="color: var(--k-text-color)"
			>
				{#each steps as step, i (i)}
					<li class="flex items-center gap-2">
						<span>{i + 1}.</span>
						{#if i === steps.length - 1}
							<span class="step-radio">
								<wa-icon src={wrapPathInSvg(mdiRadioboxMarked)}></wa-icon>
							</span>
						{/if}
						<span>{step}</span>
					</li>
				{/each}
			</ol>
			<Button rounded onClick={settings} large tonal data-testid="permission-settings-open">
				{m.settings()}
			</Button>
		</div>
	</Block>
</Sheet>

<style>
	.step-radio {
		line-height: 0;
		color: var(--color-brand-primary);
	}
	.step-radio :global(wa-icon) {
		width: 18px;
		height: 18px;
	}
</style>
