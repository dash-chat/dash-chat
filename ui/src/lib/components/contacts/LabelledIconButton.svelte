<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	type ActionHandler = () => void | Promise<void>;

	let {
		label,
		icon,
		onClick,
		testId,
	}: {
		label: string;
		icon: string;
		onClick: ActionHandler;
		testId: string;
	} = $props();
</script>

<div class="column" style="align-items: center; gap: 8px;">
	<Button tonal {onClick} class="icon-only" data-testid={testId}>
		<wa-icon src={wrapPathInSvg(icon)} style="font-size: 28px"></wa-icon>
	</Button>
	<span class="text-sm" style="color: var(--k-text-color)">{label}</span>
</div>
