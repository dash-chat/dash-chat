<script lang="ts">
	import { fileExtension } from '$lib/types/media';

	let {
		name,
		width = 32,
		height = 40,
	}: { name: string; width?: number; height?: number } = $props();

	const ext = $derived(fileExtension(name));
</script>

<div class="sheet" style="width: {width}px; height: {height}px">
	<span>{ext}</span>
</div>

<style>
	/* Signal-style document icon: a white page with the top-end corner folded
	   over (dog-ear). The corner is cut so the bubble shows through, and a
	   darker flap fills the fold. clip-path is not direction-aware, so both the
	   cut and the flap are mirrored under RTL. */
	.sheet {
		--fold: 11px;
		position: relative;
		flex-shrink: 0;
		background: white;
		border-radius: 4px;
		display: flex;
		align-items: flex-end;
		justify-content: center;
		padding-bottom: 5px;
		clip-path: polygon(
			0 0,
			calc(100% - var(--fold)) 0,
			100% var(--fold),
			100% 100%,
			0 100%
		);
	}

	:global([dir='rtl']) .sheet {
		clip-path: polygon(var(--fold) 0, 100% 0, 100% 100%, 0 100%, 0 var(--fold));
	}

	.sheet::before {
		content: '';
		position: absolute;
		top: 0;
		inset-inline-end: 0;
		width: var(--fold);
		height: var(--fold);
		background: rgba(0, 0, 0, 0.18);
		clip-path: polygon(0 0, 100% 100%, 0 100%);
	}

	:global([dir='rtl']) .sheet::before {
		clip-path: polygon(100% 0, 100% 100%, 0 100%);
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
