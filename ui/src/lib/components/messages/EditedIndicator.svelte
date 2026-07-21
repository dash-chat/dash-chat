<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';

	let {
		class: className = '',
	}: {
		class?: string;
	} = $props();
</script>

<span
	class={`text-xs/none ${className}`}
	data-testid="message-edited-indicator"
>
	{m.edited()}
</span>
