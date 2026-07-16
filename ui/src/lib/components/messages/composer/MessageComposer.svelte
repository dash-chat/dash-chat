<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, Dialog, DialogButton, useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { isMobile } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
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
	import { mdiClose, mdiPencilOutline } from '@mdi/js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import MediaDropOverlay from '$lib/components/messages/composer/MediaDropOverlay.svelte';
	import StagedAttachments from '$lib/components/messages/composer/StagedAttachments.svelte';
	import StagedMediaPage from '$lib/components/messages/composer/StagedMediaPage.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';
	import EmojiButton from '$lib/components/messages/composer/EmojiButton.svelte';
	import MediaPanel from '$lib/components/messages/composer/MediaPanel.svelte';
	import AttachMenuButton from '$lib/components/messages/composer/AttachMenuButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import IconButton from '$lib/components/IconButton.svelte';

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

	let editing = $state<Message | null>(null);
	/** Edit requested while a draft was present, awaiting discard confirmation. */
	let pendingEdit = $state<Message | null>(null);

	/** Switch the composer to editing `message`'s text instead of sending a
	 * new message. Media attachments are disabled while editing. Asks to
	 * discard first when a draft (text or staged media) would be lost. */
	export function editMessage(message: Message) {
		if (!editing && hasContent) {
			pendingEdit = message;
			return;
		}
		startEdit(message);
	}

	function startEdit(message: Message) {
		editing = message;
		value = message.content.message;
	}

	function discardDraftAndEdit() {
		const message = pendingEdit;
		pendingEdit = null;
		if (!message) return;
		media = undefined;
		startEdit(message);
	}

	function cancelEdit() {
		editing = null;
		value = '';
	}

	async function submitEdit() {
		const target = editing;
		if (!target) return;
		const text = value.trim();
		if (!text || text === target.content.message) {
			cancelEdit();
			return;
		}
		try {
			await store.editMessage(target, text);
			cancelEdit();
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

	$effect(() => {
		if (editing) messageInput?.focus();
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
		{#if !editing && !isMobile}
			<StagedAttachments bind:media onFiles={stage} />
		{/if}

		<div class="m-2 row gap-2" style="align-items: flex-end;">
			{#if editing}
				<!-- Media cannot be edited: the attach button gives way to the cancel
				     button in the narrow layout; on wide screens cancel sits after
				     the input. -->
				{#if !isWideScreen.value}
					<IconButton
						icon={mdiClose}
						circle
						onClick={cancelEdit}
						label={m.cancel()}
						testid="composer-cancel-edit"
					/>
				{/if}
			{:else if isMobile}
				<AttachButton
					class="h-10 w-10"
					expanded={showMediaPanel}
					onClick={() => (showMediaPanel = !showMediaPanel)}
				/>
			{:else}
				<EmojiButton onClick={() => (showEmojiPicker = true)} />
			{/if}
			<div
				class="input-container flex min-h-[42px] min-w-0 flex-1 flex-col justify-center {theme ===
				'ios'
					? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'bg-white dark:bg-gray-800'}"
				onpaste={onPaste}
			>
				{#if editing}
					<div
						class="flex items-center gap-1.5 ps-3 pt-2 text-sm font-semibold"
						data-testid="composer-editing-banner"
					>
						<wa-icon
							src={wrapPathInSvg(mdiPencilOutline)}
							style="font-size: 0.9rem"
						></wa-icon>
						{m.editingMessage()}
					</div>
				{/if}
				<div class="flex w-full items-center ps-2">
					<MessageInput
						bind:this={messageInput}
						bind:value
						{placeholder}
						onSend={send}
						onEmojiClick={isMobile ? () => (showEmojiPicker = true) : undefined}
					/>
				</div>
			</div>

			{#if editing && isWideScreen.value}
				<IconButton
					icon={mdiClose}
					circle
					onClick={cancelEdit}
					label={m.cancel()}
					testid="composer-cancel-edit"
				/>
			{/if}
			{#if isMobile || editing}
				<SendButton
					disabled={!hasContent}
					onSend={send}
					editing={editing !== null}
				/>
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

<Dialog
	opened={pendingEdit !== null}
	onBackdropClick={() => (pendingEdit = null)}
	title={m.discardDraftTitle()}
	data-testid="composer-discard-draft-dialog"
>
	<span>{m.discardDraftDescription()}</span>
	{#snippet buttons()}
		<DialogButton
			data-testid="composer-discard-draft-cancel"
			onClick={() => (pendingEdit = null)}
		>
			{m.cancel()}
		</DialogButton>
		<DialogButton
			data-testid="composer-discard-draft-confirm"
			onClick={discardDraftAndEdit}
		>
			{m.discard()}
		</DialogButton>
	{/snippet}
</Dialog>

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
	}
</style>
