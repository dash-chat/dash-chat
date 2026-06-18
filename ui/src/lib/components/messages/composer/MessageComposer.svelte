<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, useTheme } from 'konsta/svelte';
	import { isMobile } from '$lib/utils/environment';
	import {
		type DraftMedia,
		type IngestError,
		draftToMedia,
		ingestFiles,
		AttachmentTooLargeError,
		formatFileSize,
		MAX_MESSAGE_BYTES,
	} from '$lib/utils/media';
	import type { Hash, MessagesStore } from 'dash-chat-stores';
	import { keepKeyboardOpen } from '$lib/actions/keep-keyboard-open';
	import { showToast } from '$lib/utils/toasts';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import MediaDropOverlay from '$lib/components/messages/composer/MediaDropOverlay.svelte';
	import StagedAttachments from '$lib/components/messages/composer/StagedAttachments.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';
	import MediaPanel from '$lib/components/messages/composer/MediaPanel.svelte';
	import MediaMenu from '$lib/components/messages/composer/MediaMenu.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';

	interface Props {
		value?: string;
		placeholder?: string;
		/** The direct- or group-chat store the composer persists messages to. */
		store: MessagesStore;
		/** Called after a message is successfully sent (e.g. to scroll the chat). */
		onSent?: (messageHash: Hash) => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		store,
		onSent,
	}: Props = $props();

	const theme = $derived(useTheme());

	let media: DraftMedia | undefined = $state(undefined);
	let hasContent = $derived(value.trim().length > 0 || media !== undefined);
	let messageInput: ReturnType<typeof MessageInput> | undefined = $state();
	let showEmojiPicker = $state(false);
	let showMediaPanel = $state(false);
	let showMediaMenu = $state(false);

	function triggerSend() {
		if (!hasContent) return;
		void send();
		messageInput?.reset();
	}

	async function send() {
		const message = value;
		const draft = media;
		try {
			const wireMedia = draft ? await draftToMedia(draft) : null;
			const hash = await store.sendMessage({ message, media: wireMedia });
			// Only clear what this send actually consumed: the user may have
			// typed or staged new attachments while the send was confirming.
			if (value === message) value = '';
			if (media === draft) {
				media = undefined;
			}
			onSent?.(hash);
		} catch (e) {
			if (e instanceof AttachmentTooLargeError) {
				showToast(
					m.errorAttachmentTooLarge({
						max: formatFileSize(MAX_MESSAGE_BYTES),
					}),
					'error',
				);
				return;
			}
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to send message', e);
		}
	}

	const ingestErrorMessages: Record<IngestError, () => string> = {
		tooMany: () => m.errorTooManyAttachments(),
		filesWithPhotos: () => m.errorFilesWithPhotos(),
		oneFileAtATime: () => m.errorOneFileAtATime(),
	};

	/** Add files to the draft, toasting if a Signal mixing rule was violated. */
	function stage(files: FileList | File[]) {
		const result = ingestFiles(media, Array.from(files));
		if (result.error) showToast(ingestErrorMessages[result.error](), 'error');
		media = result.media;
	}

	function onPaste(event: ClipboardEvent) {
		const files = event.clipboardData?.files;
		if (!files || files.length === 0) return;
		event.preventDefault();
		stage(files);
	}
</script>

<MediaDropOverlay onFiles={stage} />

<div style="display: flow-root" use:keepKeyboardOpen>
	<div class="message-input-bar" class:pb-safe={!(isMobile && showMediaPanel)}>
		<StagedAttachments bind:media onFiles={stage} />

		<div class="m-2 row gap-2" style="align-items: center;">
			{#if isMobile}
				<AttachButton
					class="h-10 w-10"
					expanded={showMediaPanel}
					onClick={() => (showMediaPanel = !showMediaPanel)}
				/>
			{:else}
				<AttachButton
					class="h-10 w-10"
					expanded={showMediaMenu}
					onClick={() => (showMediaMenu = !showMediaMenu)}
				/>
			{/if}
			<div
				class="input-container flex min-h-[42px] min-w-0 flex-1 items-center ps-2 {theme ===
				'ios'
					? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'bg-white dark:bg-gray-800'}"
				onpaste={onPaste}
			>
				<MessageInput
					bind:this={messageInput}
					bind:value
					{placeholder}
					onSend={triggerSend}
					onEmojiClick={() => (showEmojiPicker = true)}
				/>
			</div>

			{#if isMobile}
				<SendButton disabled={!hasContent} onClick={triggerSend} />
			{/if}
		</div>
	</div>

	{#if isMobile}
		<MediaPanel bind:opened={showMediaPanel} onFiles={stage} />
	{/if}
</div>

{#if !isMobile}
	<MediaMenu
		bind:opened={showMediaMenu}
		target="[data-testid='message-input-attach']"
		onFiles={stage}
	/>
{/if}

<Sheet
	class="pb-safe text-lg"
	opened={showEmojiPicker}
	onBackdropClick={() => (showEmojiPicker = false)}
>
	<div class="flex flex-col items-center">
		<div class="sheet-handle"></div>
	</div>
	<Block>
		<EmojiPickerWrapper
			onEmojiSelected={emoji => {
				value += emoji;
				showEmojiPicker = false;
			}}
		></EmojiPickerWrapper>
	</Block>
</Sheet>

<style>
	.input-container {
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--color-brand-primary);
	}
</style>
