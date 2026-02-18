<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		Page,
		Navbar,
		NavbarBackLink,
		Segmented,
		SegmentedButton,
	} from 'konsta/svelte';

	// Get initial values from URL params or use defaults
	const initialText = $page.url.searchParams.get('text') || '';
	const initialColor = $page.url.searchParams.get('color') || '#fce7f3';

	let text = $state(initialText);
	let selectedColor = $state(initialColor);
	let activeTab = $state<'text' | 'color'>('text');
	let hiddenInput: HTMLInputElement;

	// Pastel color palette (similar to Signal)
	const colors = [
		'#ddd6fe', '#bfdbfe', '#cffafe', '#bbf7d0',
		'#e9d5ff', '#fbcfe8', '#fce7f3', '#fecaca',
		'#fef08a', '#d9f99d', '#e5e7eb', '#d1d5db',
	];

	onMount(() => {
		// Auto-focus the hidden input when on text tab
		if (activeTab === 'text') {
			hiddenInput?.focus();
		}
	});

	function done() {
		// Navigate back to edit-photo with the generated avatar data
		const params = new URLSearchParams();
		params.set('text', text);
		params.set('color', selectedColor);
		goto(`/settings/profile/edit-photo?${params.toString()}`);
	}

	// Limit text to 3 characters
	function handleInput(e: Event) {
		const input = e.target as HTMLInputElement;
		text = input.value.slice(0, 3).toUpperCase();
	}

	function focusInput() {
		if (activeTab === 'text') {
			hiddenInput?.focus();
		}
	}

	// Focus input when switching to text tab
	$effect(() => {
		if (activeTab === 'text') {
			setTimeout(() => hiddenInput?.focus(), 100);
		}
	});
</script>

<Page>
	<Navbar title={m.preview()} transparent titleClass="opacity1">
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/settings/profile/edit-photo')} />
		{/snippet}
		{#snippet subnavbar()}
			<Segmented strong>
				<SegmentedButton
					strong
					active={activeTab === 'text'}
					onClick={() => (activeTab = 'text')}
				>
					{m.text()}
				</SegmentedButton>
				<SegmentedButton
					strong
					active={activeTab === 'color'}
					onClick={() => (activeTab = 'color')}
				>
					{m.color()}
				</SegmentedButton>
			</Segmented>
		{/snippet}
	</Navbar>

	<!-- Hidden input for capturing text -->
	<input
		type="text"
		class="hidden-input"
		bind:this={hiddenInput}
		value={text}
		oninput={handleInput}
		maxlength="3"
		onblur={() => activeTab === 'text' && setTimeout(() => hiddenInput?.focus(), 0)}
	/>

	<div class="column" style="flex: 1; overflow-y: auto;">
		<!-- Avatar preview (clickable to focus input on text tab) -->
		<div class="column" style="align-items: center; padding: 40px 0;">
			<button
				class="avatar-preview"
				style="background-color: {selectedColor};"
				onclick={focusInput}
				type="button"
			>
				{#if activeTab === 'text'}
					<span class="avatar-text">{text}<span class="avatar-cursor">|</span></span>
				{:else}
					<span class="avatar-text">{text}</span>
				{/if}
			</button>
		</div>

		<!-- Tab content -->
		{#if activeTab === 'color'}
			<!-- Color grid -->
			<div class="color-grid">
				{#each colors as color}
					<button
						class="color-btn"
						class:selected={selectedColor === color}
						style="background-color: {color};"
						onclick={() => (selectedColor = color)}
					>
					</button>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Done button -->
	<Button
		rounded
		tonal
		onClick={done}
		style="position: fixed; bottom: 16px; right: 16px; width: auto"
	>
		{m.done()}
	</Button>
</Page>

<style>
	.hidden-input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.avatar-preview {
		width: 180px;
		height: 180px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		cursor: pointer;
	}

	.avatar-text {
		font-size: 56px;
		font-weight: 500;
		color: #831843;
	}

	.avatar-cursor {
		font-size: 56px;
		font-weight: 300;
		color: #831843;
		animation: blink 1s infinite;
		margin-left: -2px;
	}

	@keyframes blink {
		0%, 50% { opacity: 1; }
		51%, 100% { opacity: 0; }
	}

	.color-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 16px;
		padding: 24px 24px 80px;
		justify-items: center;
	}

	.color-btn {
		width: 72px;
		height: 72px;
		border-radius: 50%;
		border: 3px solid transparent;
		cursor: pointer;
		transition: transform 0.2s;
	}

	.color-btn.selected {
		border-color: #374151;
	}

	.color-btn:hover {
		transform: scale(1.05);
	}

	.color-btn:active {
		transform: scale(0.95);
	}
</style>
