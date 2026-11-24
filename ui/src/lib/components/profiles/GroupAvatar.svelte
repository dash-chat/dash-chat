<script lang="ts">
	import { mdiAccountGroupOutline } from '@mdi/js';
	import Avatar from './Avatar.svelte';

	let {
		image,
		size,
		testId,
	}: {
		image?: string | undefined;
		size?: number | string | undefined;
		testId?: string | undefined;
	} = $props();
</script>

<Avatar {image} fallbackIcon={mdiAccountGroupOutline} {size} {testId} />
