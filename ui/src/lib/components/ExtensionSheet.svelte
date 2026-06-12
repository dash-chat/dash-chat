<script lang="ts">
	import { fileExtension } from '$lib/types/media';

	let {
		name,
		width = 30,
		height = 40,
	}: { name: string; width?: number; height?: number } = $props();

	const ext = $derived(fileExtension(name));
</script>

<div class="sheet" style="width: {width}px; height: {height}px">
	<span>{ext}</span>
</div>

<style>
	.sheet {
		flex-shrink: 0;
		background: white;
		border: 1px solid rgba(0, 0, 0, 0.15);
		border-radius: 3px;
		display: flex;
		align-items: flex-end;
		justify-content: center;
		padding-bottom: 3px;
	}

	.sheet span {
		font-size: 10px;
		font-weight: 700;
		color: #5e5e5e;
		line-height: 1;
		max-width: 100%;
		overflow: hidden;
	}
</style>
