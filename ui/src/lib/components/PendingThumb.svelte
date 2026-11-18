<script lang="ts">
	import { Preloader } from 'konsta/svelte';

	let {
		class: klass = '',
		spinnerClass = '',
	}: { class?: string; spinnerClass?: string } = $props();
</script>

<div
	class="flex shrink-0 items-center justify-center {klass}"
	data-testid="staged-photo-pending"
>
	<Preloader class="h-6 w-6 {spinnerClass}" />
</div>
