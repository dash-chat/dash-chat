<script lang="ts">
	let theme = $state<'ios' | 'material'>('material');
	let dark = $state(false);
	let mobile = $state(false);
	let open = $state(false);
	let updater = $state(false);

	function toggleTheme() {
		theme = theme === 'ios' ? 'material' : 'ios';
		window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme } }));
	}

	function toggleDark() {
		dark = !dark;
		window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: dark }));
	}

	function toggleLayout() {
		mobile = !mobile;
		window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: !mobile }));
		window.dispatchEvent(new CustomEvent('set-mobile-frame', { detail: mobile }));
	}

	function toggleUpdater() {
		updater = !updater;
		window.dispatchEvent(
			new CustomEvent('test-simulate-update', { detail: updater ? 'downloading' : 'idle' }),
		);
	}

	function resetData() {
		localStorage.clear();
		window.location.reload();
	}
</script>

<div class="preview-float">
	<button class="preview-toggle" onclick={() => (open = !open)}>
		{open ? '✕' : '⚙'}
	</button>
	{#if open}
		<div class="preview-panel">
			<button class="preview-btn" onclick={toggleTheme}>
				{theme === 'ios' ? 'iOS' : 'Material'}
			</button>
			<button class="preview-btn" onclick={toggleDark}>
				{dark ? 'Dark' : 'Light'}
			</button>
			<button class="preview-btn" onclick={toggleLayout}>
				{mobile ? 'Mobile' : 'Desktop'}
			</button>
			<button class="preview-btn" onclick={toggleUpdater}>
				{updater ? 'Hide Update' : 'Show Update'}
			</button>
			<button class="preview-btn preview-btn-reset" onclick={resetData}>
				Reset
			</button>
		</div>
	{/if}
</div>

<style>
	.preview-float {
		position: fixed;
		bottom: 16px;
		left: 16px;
		z-index: 99999;
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 8px;
		font-family: -apple-system, 'Segoe UI', Roboto, sans-serif;
	}
	.preview-panel {
		display: flex;
		flex-direction: row;
		gap: 4px;
		background: #1a1a2e;
		border-radius: 8px;
		padding: 8px;
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.4);
	}
	.preview-toggle {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		background: #1a1a2e;
		color: #e0e0e0;
		border: 1px solid #3a3a5a;
		font-size: 18px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
		flex-shrink: 0;
	}
	.preview-toggle:hover {
		background: #2a2a4a;
	}
	.preview-btn {
		background: #2a2a4a;
		color: #e0e0e0;
		border: 1px solid #3a3a5a;
		border-radius: 4px;
		padding: 4px 14px;
		font-size: 12px;
		cursor: pointer;
		font-family: inherit;
		transition: background 0.15s;
		white-space: nowrap;
	}
	.preview-btn:hover {
		background: #3a3a5a;
	}
	.preview-btn-reset {
		background: #4a2020;
		border-color: #6a3030;
	}
	.preview-btn-reset:hover {
		background: #6a3030;
	}
</style>
