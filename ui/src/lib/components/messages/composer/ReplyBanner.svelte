<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiClose, mdiReply } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { type Message, hasBody } from 'dash-chat-stores';

	let {
		message,
		authorName,
		onCancel,
	}: {
		message: Message;
		/** Display name of the author being replied to. */
		authorName: string;
		onCancel: () => void;
	} = $props();

	const body = $derived(hasBody(message.content) ? message.content : null);
	const preview = $derived(
		body?.message ||
			(body?.media?.kind === 'photos'
				? m.photo()
				: (body?.media?.file.name ?? '')),
	);
</script>

<div
	class="row items-center gap-2 ps-3 pe-1 pt-2 text-sm"
	data-testid="composer-reply-banner"
>
	<wa-icon src={wrapPathInSvg(mdiReply)} style="font-size: 0.9rem"></wa-icon>
	<span class="column min-w-0 flex-1">
		<span class="truncate font-semibold">
			{m.replyingTo({ name: authorName })}
		</span>
		<span class="quiet truncate" data-testid="composer-reply-preview">
			{preview}
		</span>
	</span>
	<button
		type="button"
		class="quiet flex h-7 w-7 items-center justify-center"
		aria-label={m.cancel()}
		data-testid="composer-cancel-reply"
		onclick={onCancel}
	>
		<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 1.1rem"></wa-icon>
	</button>
</div>
