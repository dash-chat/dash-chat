<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiClose } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		type Message,
		hasBody,
		mediaBundleToAttachment,
	} from 'dash-chat-stores';
	import IconButton from '$lib/components/IconButton.svelte';
	import QuoteFrame from '$lib/components/messages/QuoteFrame.svelte';

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
	const media = $derived(mediaBundleToAttachment(body?.media));
	const preview = $derived(
		body?.message ||
			(media?.kind === 'photos'
				? m.photo()
				: media?.kind === 'file'
					? media.file.name
					: media?.kind === 'voice_note'
						? m.voiceMessage()
						: ''),
	);
</script>

<div
	class="row items-center px-2 pt-2 text-sm"
	data-testid="composer-reply-banner"
>
	<QuoteFrame>
		<span class="row min-w-0 flex-1 items-center gap-2 ps-2 py-1.5">
			<span class="column min-w-0 flex-1">
				<span class="truncate font-semibold">
					{m.replyingTo({ name: authorName })}
				</span>
				<span class="quiet truncate" data-testid="composer-reply-preview">
					{preview}
				</span>
			</span>
		</span>
		<IconButton
			label={m.cancel()}
			testid="composer-cancel-reply"
			onClick={onCancel}
			class="!h-7 !w-7 shrink-0 self-start"
		>
			<wa-icon class="text-xl" src={wrapPathInSvg(mdiClose)}></wa-icon>
		</IconButton>
	</QuoteFrame>
</div>
