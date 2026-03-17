<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiArrowLeft } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		Link,
		Navbar,
		Segmented,
		SegmentedButton,
	} from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';

	let {
		avatar = $bindable(),
		onSelect,
		onClose,
	}: {
		avatar?: string | undefined;
		onSelect?: () => void;
		onClose?: () => void;
	} = $props();

	let textValue = $state('');
	let selectedColor = $state('#fce7f3');
	let activeTab = $state<'text' | 'color'>('text');
	let hiddenInput: HTMLInputElement;

	const colors = [
		'#ddd6fe',
		'#bfdbfe',
		'#cffafe',
		'#bbf7d0',
		'#e9d5ff',
		'#fbcfe8',
		'#fce7f3',
		'#fecaca',
		'#fef08a',
		'#d9f99d',
		'#e5e7eb',
		'#d1d5db',
	];

	function generateTextAvatar() {
		avatar = `data:text/plain;charset=utf-8,${selectedColor}|${encodeURIComponent(textValue.toUpperCase())}`;
		console.log('Generated text avatar:', avatar);
		onSelect?.();
	}

	function handleTextInput(e: Event) {
		const input = e.target as HTMLInputElement;
		textValue = input.value.slice(0, 3).toUpperCase();
	}

	function focusTextInput() {
		if (activeTab === 'text') {
			hiddenInput?.focus();
		}
	}

	$effect(() => {
		if (activeTab === 'text') {
			setTimeout(() => hiddenInput?.focus(), 100);
		}
	});
</script>

<input
	type="text"
	class="absolute opacity-0 pointer-events-none"
	bind:this={hiddenInput}
	value={textValue}
	oninput={handleTextInput}
	maxlength="3"
	onblur={() =>
		activeTab === 'text' && setTimeout(() => hiddenInput?.focus(), 0)}
/>

<!-- Text avatar editor -->
<Navbar transparent rightClass={!textValue ? 'ios-right-disabled' : ''}>
	{#snippet left()}
		<Link iconOnly onClick={onClose} data-testid="edit-photo-back">
			<wa-icon src={wrapPathInSvg(mdiArrowLeft)} style="font-size: 24px"
			></wa-icon>
		</Link>
	{/snippet}
	{#snippet right()}
		{#if isIos}
			<Link onClick={generateTextAvatar}>
				{m.done()}
			</Link>
		{/if}
	{/snippet}
</Navbar>

<div style="padding: 0 16px 16px;">
	<Segmented strong>
		<SegmentedButton
			active={activeTab === 'text'}
			onClick={() => (activeTab = 'text')}
		>
			{m.text()}
		</SegmentedButton>
		<SegmentedButton
			active={activeTab === 'color'}
			onClick={() => (activeTab = 'color')}
		>
			{m.color()}
		</SegmentedButton>
	</Segmented>
</div>

<!-- Text avatar preview -->
<div class="column" style="align-items: center; padding: 24px 0;">
	<button
		class="w-[180px] h-[180px] rounded-full flex items-center justify-center border-none cursor-pointer"
		style="background-color: {selectedColor};"
		onclick={focusTextInput}
		type="button"
	>
		{#if activeTab === 'text'}
			<span class="text-[56px] font-medium text-pink-900"
				>{textValue}<span
					class="text-[56px] font-light text-pink-900 animate-[blink_1s_infinite] -ml-0.5"
					>|</span
				></span
			>
		{:else}
			<span class="text-[56px] font-medium text-pink-900">{textValue}</span>
		{/if}
	</button>
</div>

{#if activeTab === 'color'}
	<div class="grid grid-cols-4 gap-4 px-6 py-6 justify-items-center">
		{#each colors as color}
			<button
				class="w-[72px] h-[72px] rounded-full border-[3px] cursor-pointer transition-transform duration-200 hover:scale-105 active:scale-95 {selectedColor ===
				color
					? 'border-gray-700'
					: 'border-transparent'}"
				style="background-color: {color};"
				aria-label="Select color {color}"
				onclick={() => (selectedColor = color)}
			>
			</button>
		{/each}
	</div>
{/if}

{#if !isIos}
	<Button
		rounded
		tonal
		disabled={!textValue}
		onClick={generateTextAvatar}
		class="fixed-action-btn"
	>
		{m.done()}
	</Button>
{/if}

<style>
	@keyframes blink {
		0%,
		50% {
			opacity: 1;
		}
		51%,
		100% {
			opacity: 0;
		}
	}
</style>
