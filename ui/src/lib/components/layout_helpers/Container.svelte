<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children, class: className }: { children: Snippet; class?: string } =
		$props();
</script>

<div class="column">
	<div class="center-in-desktop {className}">
		{@render children()}
	</div>
</div>
