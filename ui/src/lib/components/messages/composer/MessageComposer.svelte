<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { isMobile } from '$lib/utils/environment';
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
	import type { Hash, Message, MessagesStore } from 'dash-chat-stores';
	import { keepKeyboardOpen } from '$lib/actions/keep-keyboard-open';
	import { showToast } from '$lib/utils/toasts';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiPencil, mdiReply } from '@mdi/js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import MediaDropOverlay from '$lib/components/messages/composer/MediaDropOverlay.svelte';
	import StagedAttachments from '$lib/components/messages/composer/StagedAttachments.svelte';
	import StagedMediaPage from '$lib/components/messages/composer/StagedMediaPage.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';
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
		/** When set, the composer edits this message's text instead of sending a
		 * new message. Media attachments are disabled while editing. */
		editing?: Message | null;
		/** Submit an edit of `message` with the new `text`. */
		onEdit?: (message: Message, text: string) => Promise<void>;
		/** Called when the user cancels an in-progress edit. */
		onCancelEdit?: () => void;
		/** When set, the next send is a reply to this message. */
		replying?: Message | null;
		/** Display name of the author being replied to, for the banner. */
		replyingToName?: string;
		/** Called when the user cancels the staged reply (also after sending). */
		onCancelReply?: () => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		store,
		destinationName,
		onSent,
		editing = null,
		onEdit,
		onCancelEdit,
		replying = null,
		replyingToName = '',
		onCancelReply,
	}: Props = $props();

	const theme = $derived(useTheme());

	let media: DraftMedia | undefined = $state(undefined);
	let hasContent = $derived(value.trim().length > 0 || media !== undefined);
	let messageInput: ReturnType<typeof MessageInput> | undefined = $state();
	let showEmojiPicker = $state(false);
	let sending = false;

	let showMediaPanel = $state(false);

	async function submitEdit() {
		const target = editing;
		if (!target || !onEdit) return;
		const text = value.trim();
		if (!text || text === target.content.message) {
			onCancelEdit?.();
			return;
		}
		try {
			await onEdit(target, text);
			onCancelEdit?.();
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to edit message', e);
		}
	}

	/** Returns whether the message was sent (so callers can keep the draft on failure). */
	async function send(): Promise<boolean> {
		if (editing) {
			await submitEdit();
			return false;
		}
		// Guard against concurrent sends: the button shows a spinner, but the
		// Enter-key path goes straight here, so hammering Enter during a slow
		// send would otherwise fire multiple store.sendMessage calls.
		if (!hasContent || sending) return false;
		sending = true;
		const message = value;
		const draft = media;
		const replyTo = replying;
		try {
			const wireMedia = draft ? await draftToMedia(draft) : null;
			const hash = await store.sendMessage({
				message,
				media: wireMedia,
				replyTo,
			});
			// Only clear what this send actually consumed: the user may have
			// typed or staged new attachments while the send was confirming.
			if (value === message) value = '';
			if (media === draft) {
				media = undefined;
			}
			if (replyTo) onCancelReply?.();
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
		{#if editing}
			<div
				class="row items-center gap-2 px-3 pt-2 text-sm"
				data-testid="composer-editing-banner"
			>
				<wa-icon
					class="quiet"
					src={wrapPathInSvg(mdiPencil)}
					style="font-size: 1rem"
				></wa-icon>
				<span class="flex-1 quiet truncate">{m.editingMessage()}</span>
				<button
					type="button"
					class="quiet flex h-7 w-7 items-center justify-center"
					aria-label={m.cancel()}
					data-testid="composer-cancel-edit"
					onclick={() => onCancelEdit?.()}
				>
					<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 1.1rem"
					></wa-icon>
				</button>
			</div>
		{:else}
			{#if replying}
				<div
					class="row items-center gap-2 px-3 pt-2 text-sm"
					data-testid="composer-reply-banner"
				>
					<wa-icon
						class="quiet"
						src={wrapPathInSvg(mdiReply)}
						style="font-size: 1rem"
					></wa-icon>
					<span class="column min-w-0 flex-1">
						<span class="quiet truncate font-semibold">
							{m.replyingTo({ name: replyingToName })}
						</span>
						<span class="quiet truncate" data-testid="composer-reply-preview">
							{replying.content.message ||
								(replying.content.media?.kind === 'photos'
									? m.photo()
									: (replying.content.media?.file.name ?? ''))}
						</span>
					</span>
					<button
						type="button"
						class="quiet flex h-7 w-7 items-center justify-center"
						aria-label={m.cancel()}
						data-testid="composer-cancel-reply"
						onclick={() => onCancelReply?.()}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 1.1rem"
						></wa-icon>
					</button>
				</div>
			{/if}
			{#if !isMobile}
				<StagedAttachments bind:media onFiles={stage} />
			{/if}
		{/if}

		<div class="m-2 row gap-2" style="align-items: center;">
			{#if editing}
				<!-- Media cannot be edited, so the attach button is hidden. -->
			{:else if isMobile}
				<AttachButton
					class="h-10 w-10"
					expanded={showMediaPanel}
					onClick={() => (showMediaPanel = !showMediaPanel)}
				/>
			{:else}
				<AttachMenuButton onFiles={stage} />
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
					onSend={send}
					onEmojiClick={() => (showEmojiPicker = true)}
				/>
			</div>

			{#if isMobile}
				<SendButton disabled={!hasContent} onSend={send} />
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
