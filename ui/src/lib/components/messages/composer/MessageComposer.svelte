<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { isIos, isMobile } from '$lib/utils/environment';
	import { keyboard } from '$lib/utils/keyboard.svelte';
	import {
		type DraftMedia,
		type IngestError,
		draftToMedia,
		ingestFiles,
		pickMedia,
		AttachmentTooLargeError,
		formatFileSize,
		MAX_MESSAGE_BYTES,
	} from '$lib/utils/media';
	import type { Hash, MessagesStore } from 'dash-chat-stores';
	import { keepKeyboardOpen } from '$lib/actions/keep-keyboard-open';
	import { showToast } from '$lib/utils/toasts';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import MediaDropOverlay from '$lib/components/messages/composer/MediaDropOverlay.svelte';
	import StagedAttachments from '$lib/components/messages/composer/StagedAttachments.svelte';
	import StagedMediaPage from '$lib/components/messages/composer/StagedMediaPage.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import StandaloneAttachButton from '$lib/components/messages/composer/StandaloneAttachButton.svelte';
	import InlineAttachButton from '$lib/components/messages/composer/InlineAttachButton.svelte';
	import EmojiButton from '$lib/components/messages/composer/EmojiButton.svelte';
	import MediaPanel from '$lib/components/messages/composer/MediaPanel.svelte';
	import AttachMenuButton from '$lib/components/messages/composer/AttachMenuButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';

	interface Props {
		value?: string;
		placeholder?: string;
		/** The direct- or group-chat store the composer persists messages to. */
		store: MessagesStore;
		/** Name of the chat, shown in the mobile staged-media page header. */
		destinationName?: string;
		/** Called after a message is successfully sent (e.g. to scroll the chat). */
		onSent?: (messageHash: Hash) => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		store,
		destinationName,
		onSent,
	}: Props = $props();

	const theme = $derived(useTheme());

	let media: DraftMedia | undefined = $state(undefined);
	let hasContent = $derived(value.trim().length > 0 || media !== undefined);
	let messageInput: ReturnType<typeof MessageInput> | undefined = $state();
	let showEmojiPicker = $state(false);
	let sending = false;

	let showMediaPanel = $state(false);

	function toggleMediaPanel() {
		if (!showMediaPanel) {
			showMediaPanel = true;
			return;
		}
		// Don't close the panel here: focusing the input makes renderBelowKeyboard
		// yield the panel's slot to the rising keyboard in lockstep (keeping the
		// input bar pinned) and clear `showMediaPanel` once the swap completes —
		// including when no keyboard rises (its yield backstop).
		messageInput?.focus();
	}

	/** Returns whether the message was sent (so callers can keep the draft on failure). */
	async function send(): Promise<boolean> {
		// Guard against concurrent sends: the button shows a spinner, but the
		// Enter-key path goes straight here, so hammering Enter during a slow
		// send would otherwise fire multiple store.sendMessage calls.
		if (!hasContent || sending) return false;
		sending = true;
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
			messageInput?.reset();
			onSent?.(hash);
			return true;
		} catch (e) {
			if (e instanceof AttachmentTooLargeError) {
				showToast(
					m.errorAttachmentTooLarge({
						max: formatFileSize(MAX_MESSAGE_BYTES),
					}),
					'error',
				);
				return false;
			}
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to send message', e);
			return false;
		} finally {
			sending = false;
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
		if (isMobile && media && !page.state.stagedMedia) {
			pushState('', { stagedMedia: true });
		}
	}

	async function addMore() {
		try {
			const files = await pickMedia('image', true);
			if (files && files.length > 0) stage(files);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to pick files', e);
		}
	}

	// Popping the staged-media history entry (hardware/browser back or
	// `history.back()` from the page) discards the staged draft.
	$effect(() => {
		if (isMobile && media && !page.state.stagedMedia) media = undefined;
	});

	function onPaste(event: ClipboardEvent) {
		const files = event.clipboardData?.files;
		if (!files || files.length === 0) return;
		event.preventDefault();
		stage(files);
	}
</script>

<MediaDropOverlay onFiles={stage} />

{#snippet emojiButton()}
	<EmojiButton onClick={() => (showEmojiPicker = true)} />
{/snippet}

<div style="display: flow-root" use:keepKeyboardOpen>
	<!-- Safe-area padding only when the bar is the bottom-most surface (nothing
	     below it): no panel and no keyboard. Keying it off the panel alone bumps
	     the bar by `env(safe-area-inset-bottom)` during the panel→keyboard swap,
	     because the panel closes before the (visual-viewport-driven) safe area
	     has collapsed to 0. -->
	<div
		class="message-input-bar"
		class:pb-safe={!showMediaPanel && !keyboard.isOpen}
	>
		{#if !isMobile}
			<StagedAttachments bind:media onFiles={stage} />
		{/if}

		<div class="m-2 row gap-2" style="align-items: center;">
			{#if isMobile}
				{#if theme === 'ios'}
					<StandaloneAttachButton
						expanded={showMediaPanel}
						onClick={toggleMediaPanel}
					/>
				{/if}
			{:else}
				<EmojiButton onClick={() => (showEmojiPicker = true)} />
			{/if}
			<MessageInput
				bind:this={messageInput}
				bind:value
				{placeholder}
				onSend={send}
				onpaste={onPaste}
				before={isMobile && !isIos ? emojiButton : undefined}
			>
				{#snippet after()}
					{#if isMobile && theme === 'material' && hasContent}
						<InlineAttachButton
							expanded={showMediaPanel}
							onClick={toggleMediaPanel}
						/>
					{/if}
				{/snippet}
			</MessageInput>

			{#if isMobile}
				{#if hasContent}
					<SendButton onSend={send} />
				{:else if theme !== 'ios'}
					<StandaloneAttachButton
						expanded={showMediaPanel}
						onClick={toggleMediaPanel}
					/>
				{/if}
			{:else}
				<AttachMenuButton onFiles={stage} />
			{/if}
		</div>
	</div>

	{#if isMobile}
		<MediaPanel bind:open={showMediaPanel} onFiles={stage} />
	{/if}
</div>

{#if isMobile && media && page.state.stagedMedia}
	<StagedMediaPage
		bind:media
		bind:value
		{destinationName}
		onSend={async () => {
			const sent = await send();
			// Guard against the stagedMedia entry already being popped (e.g. the user
			// hit back during a slow send) — otherwise we'd navigate off the chat.
			if (sent && page.state.stagedMedia) history.back();
			return sent;
		}}
		onAddMore={addMore}
		onClose={() => history.back()}
	/>
{/if}

<Sheet
	class="pb-safe text-lg"
	opened={showEmojiPicker}
	onBackdropClick={() => (showEmojiPicker = false)}
>
	<div class="flex flex-col items-center">
		<SheetHandle />
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
