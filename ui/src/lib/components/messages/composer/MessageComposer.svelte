<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, Dialog, DialogButton, useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { isIos, isMobile } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
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
	import { renderAboveKeyboard } from '$lib/utils/virtual-keyboard/render-above-keyboard';
	import { hideKeyboard } from 'tauri-plugin-virtual-keyboard';
	import BelowKeyboardSurface from '$lib/components/BelowKeyboardSurface.svelte';
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
	import EditingBanner from '$lib/components/messages/composer/EditingBanner.svelte';
	import DiscardEditButton from '$lib/components/messages/composer/DiscardEditButton.svelte';

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
		if (!target || sending) return;
		const text = value.trim();
		if (!text || text === target.content.message) {
			cancelEdit();
			return;
		}
		sending = true;
		try {
			await store.editMessage(target, text);
			cancelEdit();
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to edit message', e);
		} finally {
			sending = false;
		}
	}

	function toggleMediaPanel() {
		if (!showMediaPanel) {
			showMediaPanel = true;
			return;
		}
		// Flip the intent right away so the attach button reacts instantly, then
		// hand focus to the input: the plugin sees the close arrive with an input
		// focused and holds the reserved inset until the rising keyboard claims the
		// slot, so the input bar stays pinned during the swap.
		showMediaPanel = false;
		messageInput?.focus();
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

	function stageFromPanel(files: File[]) {
		showMediaPanel = false;
		stage(files);
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

	function openEmojiPicker() {
		hideKeyboard();
		showEmojiPicker = true;
	}

	function onPaste(event: ClipboardEvent) {
		const files = event.clipboardData?.files;
		if (!files || files.length === 0) return;
		event.preventDefault();
		stage(files);
	}
</script>

<MediaDropOverlay onFiles={stage} />

{#snippet emojiButton()}
	<EmojiButton onClick={openEmojiPicker} />
{/snippet}

{#snippet editingBanner()}
	<EditingBanner />
{/snippet}

<div style="display: flow-root" use:keepKeyboardOpen>
	<div
		class="message-input-bar relative flow-root {theme === 'ios'
			? 'z-30'
			: 'z-10'}"
		class:bg-page-surface={theme === 'material'}
		use:renderAboveKeyboard
	>
		{#if !editing && !isMobile}
			<StagedAttachments bind:media onFiles={stage} />
		{/if}

		<div class="m-2 row gap-2" style="align-items: flex-end;">
			{#if editing}
				{#if !isWideScreen.value}
					<DiscardEditButton onClick={cancelEdit} />
				{/if}
			{:else if isMobile && theme === 'ios'}
				<StandaloneAttachButton
					expanded={showMediaPanel}
					onClick={toggleMediaPanel}
				/>
			{/if}
			{#if !isMobile}
				<EmojiButton onClick={openEmojiPicker} />
			{/if}
			<MessageInput
				bind:this={messageInput}
				bind:value
				{placeholder}
				onSend={send}
				onpaste={onPaste}
				onfocus={() => (showMediaPanel = false)}
				before={isMobile && !isIos ? emojiButton : undefined}
				banner={editing !== null ? editingBanner : undefined}
			>
				{#snippet after()}
					{#if !editing && isMobile && theme === 'material' && hasContent}
						<InlineAttachButton
							expanded={showMediaPanel}
							onClick={toggleMediaPanel}
						/>
					{/if}
				{/snippet}
			</MessageInput>

			{#if editing}
				{#if isWideScreen.value}
					<DiscardEditButton onClick={cancelEdit} />
				{/if}
				<SendButton onSend={send} editing />
			{:else if isMobile}
				{#if isIos}
					<div
						class="flex shrink-0 items-center justify-end transition-all duration-200 ease-out {hasContent
							? 'ms-0 w-[42px] opacity-100'
							: '-ms-2 w-0 opacity-0'}"
						style="transform: scale({hasContent ? 1 : 0})"
						aria-hidden={!hasContent}
					>
						<SendButton onSend={send} />
					</div>
				{:else if hasContent}
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
		<BelowKeyboardSurface open={showMediaPanel} class="bg-page-surface z-20">
			<MediaPanel
				onFiles={stageFromPanel}
				onPickerOpen={() => (showMediaPanel = false)}
			/>
		</BelowKeyboardSurface>
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
	/* During keyboard glides the bar can lead the keyboard's edge by a few px;
	   this skirt extends the bar's surface downward so the sliver between the
	   bar and the keyboard paints page-surface instead of exposing the messages
	   gliding behind it. Invisible at rest: everything legitimately below the
	   bar (the shell's reserved-space padding, the media panel, the keyboard
	   itself) either shares this color or paints above it. */
	.message-input-bar:global(.bg-page-surface)::after {
		content: '';
		position: absolute;
		inset-inline: 0;
		top: 100%;
		height: 64px;
		background: inherit;
	}
</style>
