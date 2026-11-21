<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { boldToHtml, escapeHtml } from '$lib/utils/banner-text';

	interface Props {
		name: string;
	}

	let { name }: Props = $props();
</script>

<!-- TODO: pre-accept: stands in for the composer while our contact request
is unanswered, because the chat has no space to publish into until the peer
accepts. Remove once pre-accept messages can be sent. -->
<div class="px-6 py-3" data-testid="direct-chat-awaiting-accept">
	<p
		class="text-center text-sm text-gray-600 dark:text-gray-400 break-words min-w-0 max-w-full"
	>
		{@html boldToHtml(m.awaitingAcceptanceBanner({ name: escapeHtml(name) }))}
	</p>
</div>
