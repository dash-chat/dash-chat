<script lang="ts">
	import type { ChatId } from 'dash-chat-stores';
	import { Chip } from 'konsta/svelte';
	import ConnectionStatusIcon from './ConnectionStatusIcon.svelte';

	let { chatId }: { chatId: ChatId } = $props();
</script>

<!-- The icon mounts nothing while pending, which would leave an empty pill. -->
<Chip
	class="p-1 not-has-[[data-testid=connection-status]]:hidden"
	colors={{
		fillBgIos: 'bg-black/10 dark:bg-brand-primary',
		fillBgMaterial: 'bg-md-light-secondary-container dark:bg-brand-primary',
	}}
>
	<ConnectionStatusIcon {chatId} />
</Chip>
