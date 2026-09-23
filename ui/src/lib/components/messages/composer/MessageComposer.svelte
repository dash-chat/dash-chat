<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Block, useTheme } from 'konsta/svelte';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { isIos, isMobile } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import {
		type DraftMedia,
		type IngestError,
		capturePhoto,
		draftToMedia,
		ingestFiles,
		pickMedia,
		AttachmentTooLargeError,
		formatFileSize,
		MAX_MESSAGE_BYTES,
	} from '$lib/utils/media';
	import VoiceRecordButton from '$lib/components/messages/composer/voice/VoiceRecordButton.svelte';
	import VoiceRecordingBar from '$lib/components/messages/composer/voice/VoiceRecordingBar.svelte';
	import { VoiceRecorder } from '$lib/components/messages/composer/voice/voice-recorder.svelte';
	import {
		type Hash,
		type Message,
		type MessagesStore,
		hasBody,
	} from 'dash-chat-stores';
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
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';
	import CameraButton from '$lib/components/messages/composer/CameraButton.svelte';
	import EmojiButton from '$lib/components/messages/composer/EmojiButton.svelte';
	import MediaPanel from '$lib/components/messages/composer/MediaPanel.svelte';
	import AttachMenuButton from '$lib/components/messages/composer/AttachMenuButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import EditingBanner from '$lib/components/messages/composer/EditingBanner.svelte';
	import ReplyBanner from '$lib/components/messages/composer/ReplyBanner.svelte';
	import DiscardEditButton from '$lib/components/messages/composer/DiscardEditButton.svelte';
	import DiscardDraftDialog from '$lib/components/messages/composer/DiscardDraftDialog.svelte';

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
	/** When set, the next send is a reply to this message. */
	let replying = $state<Message | null>(null);
	/** Display name of the author being replied to, for the banner. */
	let replyingToName = $state('');
	let discardDialog: ReturnType<typeof DiscardDraftDialog> | undefined =
		$state();

	/** Switch the composer to editing `message`'s text instead of sending a
	 * new message. Media attachments are disabled while editing. Asks to
	 * discard first when a draft (text or staged media) would be lost. */
	export function editMessage(message: Message) {
		if (!editing && hasContent) {
			discardDialog?.confirm(message);
			return;
		}
		startEdit(message);
	}

	function startEdit(message: Message) {
		if (!hasBody(message.content)) return;
		// Replying and editing are mutually exclusive composer states.
		replying = null;
		editing = message;
		value = message.content.message;
	}

	function discardDraftAndEdit(message: Message) {
		media = undefined;
		startEdit(message);
	}

	function cancelEdit() {
		editing = null;
		value = '';
	}

	async function submitEdit() {
		const target = editing;
		if (!target || sending || !hasBody(target.content)) return;
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

	/** Stage `message` as the target of the next send. `authorName` is the
	 * display name shown in the banner. */
	export function replyToMessage(message: Message, authorName: string) {
		if (editing) cancelEdit();
		replying = message;
		replyingToName = authorName;
		messageInput?.focus();
	}

	function cancelReply() {
		replying = null;
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
			if (replying === replyTo) replying = null;
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

	async function captureFromCamera() {
		try {
			const file = await capturePhoto();
			if (file) stage([file]);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to capture photo', e);
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
	// `history.back()` from the page) discards the staged draft. Voice notes are
	// exempt: they never open the staged-media page, so they never push that
	// history entry, and the rule would discard every voice draft the moment it
	// was staged.
	$effect(() => {
		if (
			isMobile &&
			media &&
			media.kind !== 'voice_note' &&
			!page.state.stagedMedia
		)
			media = undefined;
	});

	$effect(() => {
		if (editing) messageInput?.focus();
	});

	// A recorded voice note sends itself the moment it lands, so it is never a
	// draft the user acts on. Tracking that separately keeps the composer from
	// morphing into its drafting layout — mic out, send button in — for the
	// length of the send and straight back again.
	let sendingVoiceNote = $state(false);

	const voice = new VoiceRecorder(async draft => {
		media = { kind: 'voice_note', voice: draft };
		sendingVoiceNote = true;
		try {
			await send();
		} finally {
			sendingVoiceNote = false;
		}
	});

	/** Whether the composer holds something the user still has to send. */
	const drafting = $derived(hasContent && !sendingVoiceNote);

	const showVoiceButton = $derived(!editing && !drafting);
	let voiceBarLeaving = $state(false);
	// The hold/locked bars are translucent on iOS and leave the trailing slot
	// open, so the input row must not show through while they're up — including
	// while the bar is still playing its exit transition back onto the input.
	const recordingCoversInput = $derived(
		voice.view === 'hold' || voice.view === 'locked' || voiceBarLeaving,
	);

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

	// Test-only: the native recorder can’t capture in the headless e2e harness.
	onMount(() => {
		const handler = (event: Event) => {
			const detail = (event as CustomEvent).detail;
			media = {
				kind: 'voice_note',
				voice: {
					bytes: new Uint8Array(detail.bytes),
					mimeType: detail.mimeType ?? 'audio/wav',
					durationMs: detail.durationMs,
					waveform: new Uint8Array(detail.waveform),
				},
			};
		};
		window.addEventListener('test-inject-voice-message', handler);
		return () =>
			window.removeEventListener('test-inject-voice-message', handler);
	});
</script>

<MediaDropOverlay onFiles={stage} />

{#snippet emojiButton()}
	<EmojiButton onClick={openEmojiPicker} />
{/snippet}

{#snippet composerActions()}
	{#if !editing}
		{#if theme === 'material'}
			<!-- Signal gives the inline plus its own HidingLinearLayout, pinned to
			     the same end as the quick toggle, so neither shifts the other. -->
			<div
				class="composer-toggle composer-toggle-end-pivot absolute bottom-0 end-1 flex items-center"
				class:composer-toggle-hidden={!drafting}
				style="--toggle-width: 2.5rem; --toggle-hidden-transform: scaleX(0.5)"
				aria-hidden={!drafting}
			>
				<AttachButton expanded={showMediaPanel} onClick={toggleMediaPanel} />
			</div>
		{/if}
		<div
			class="composer-toggle flex shrink-0 items-center"
			class:composer-toggle-smooth={theme === 'ios'}
			class:composer-toggle-end-pivot={theme !== 'ios'}
			class:composer-toggle-hidden={drafting}
			style="--toggle-width: 5rem; {theme === 'ios'
				? '--toggle-duration: 250ms; --toggle-ease: cubic-bezier(0.25, 0.1, 0.25, 1); --toggle-hidden-transform: scale(0.1)'
				: '--toggle-hidden-width: 2.5rem; --toggle-hidden-transform: scaleX(0.5)'}"
			aria-hidden={drafting}
		>
			<CameraButton onClick={captureFromCamera} />
			<VoiceRecordButton {voice} />
		</div>
	{/if}
{/snippet}

{#snippet editingBanner()}
	<EditingBanner />
{/snippet}

{#snippet replyBanner()}
	{#if replying}
		<ReplyBanner
			message={replying}
			authorName={replyingToName}
			onCancel={cancelReply}
		/>
	{/if}
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

		<div class="m-2 relative">
			<VoiceRecordingBar
				{voice}
				onLeavingChange={leaving => (voiceBarLeaving = leaving)}
			/>

			<div
				class="input-row row gap-2"
				class:covered={recordingCoversInput}
				class:control-inset={theme !== 'ios'}
				style="align-items: flex-end"
			>
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
					hidden={recordingCoversInput}
					before={isMobile && !isIos ? emojiButton : undefined}
					banner={editing !== null ? editingBanner : replyBanner}
					after={isMobile ? composerActions : undefined}
				/>

				{#if editing}
					{#if isWideScreen.value}
						<DiscardEditButton onClick={cancelEdit} />
					{/if}
					<SendButton onSend={send} editing />
				{:else if isMobile}
					{#if voice.view === 'locked'}
						<div class="visible shrink-0">
							<SendButton
								onSend={() => voice.stopAndSend()}
								testid="voice-send"
							/>
						</div>
					{:else if isIos}
						<div
							class="ios-send flex shrink-0 items-center justify-end {drafting
								? 'ms-0 w-[42px]'
								: '-ms-2 w-0'}"
							class:ios-send-hidden={!drafting}
							aria-hidden={!drafting}
						>
							<SendButton onSend={send} />
						</div>
					{:else}
						<div class="toggle-slot shrink-0">
							<div class="toggle-child" class:toggle-hidden={!drafting}>
								<SendButton onSend={send} />
							</div>
							<div class="toggle-child" class:toggle-hidden={drafting}>
								<StandaloneAttachButton
									expanded={showMediaPanel}
									onClick={toggleMediaPanel}
								/>
							</div>
						</div>
					{/if}
				{:else}
					<div
						class="composer-toggle composer-toggle-smooth flex shrink-0 items-center"
						class:composer-toggle-hidden={drafting}
						style="--toggle-width: 2.5rem; --toggle-hidden-transform: scale(0.6)"
						aria-hidden={drafting}
					>
						<VoiceRecordButton {voice} />
					</div>
					<AttachMenuButton onFiles={stage} />
				{/if}
			</div>
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
			const keepFocus = document.activeElement instanceof HTMLTextAreaElement;
			const sent = await send();
			// Guard against the stagedMedia entry already being popped (e.g. the user
			// hit back during a slow send) — otherwise we'd navigate off the chat.
			if (sent && page.state.stagedMedia) {
				// Hand focus to the composer's input before the staged page unmounts
				// so an open keyboard stays open back in the chat.
				if (keepFocus) messageInput?.focus();
				history.back();
			}
			return sent;
		}}
		onAddMore={addMore}
		onClose={() => history.back()}
	/>
{/if}

<DiscardDraftDialog bind:this={discardDialog} onConfirm={discardDraftAndEdit} />

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
	.input-row.covered {
		visibility: hidden;
	}

	/* The round controls are 40px in Material's 44px row, so flex-end alone would
	   drop them 2px below the input pill's axis. The row reserves that inset and
	   the pill opts back out of it. iOS sizes both at 40 and needs none. */
	.input-row.control-inset {
		padding-block-end: 2px;
	}

	.input-row.control-inset > :global(.input-container) {
		margin-block-end: -2px;
	}

	/* `--toggle-width` has to be explicit for the collapse to interpolate at all.
	   The slot is held for the whole fade and given back in one step, the way
	   Signal's ViewUtil.animateOut only applies GONE once the animation ends. */
	.composer-toggle {
		--toggle-duration: 150ms;
		--toggle-ease: cubic-bezier(0.4, 0, 0.2, 1);
		width: var(--toggle-width);
		overflow: hidden;
		transition:
			opacity var(--toggle-duration) var(--toggle-ease),
			transform var(--toggle-duration) var(--toggle-ease),
			width 0ms,
			visibility 0ms;
	}

	/* Signal's HidingLinearLayout pivots the squeeze on its end edge
	   (ScaleAnimation(1, 0.5f, 1, 1, RELATIVE_TO_SELF, 1f, RELATIVE_TO_SELF, 0.5f)),
	   so the group collapses towards the pill's edge rather than its own middle. */
	.composer-toggle.composer-toggle-end-pivot {
		transform-origin: 100% 50%;
	}

	:global([dir='rtl']) .composer-toggle.composer-toggle-end-pivot {
		transform-origin: 0% 50%;
	}

	.composer-toggle.composer-toggle-hidden {
		width: var(--toggle-hidden-width, 0);
		visibility: hidden;
		opacity: 0;
		transform: var(--toggle-hidden-transform);
		transition-delay: 0ms, 0ms, var(--toggle-duration), var(--toggle-duration);
	}

	/* Releasing the slot in one step only goes unseen inside the pill, which is
	   flex-1 and so keeps its width either way. Outside it the pill would snap
	   wider, so collapse the slot on the same curve as the fade. */
	.composer-toggle.composer-toggle-smooth {
		transition:
			opacity var(--toggle-duration) var(--toggle-ease),
			transform var(--toggle-duration) var(--toggle-ease),
			width var(--toggle-duration) var(--toggle-ease),
			visibility 0ms;
	}

	.composer-toggle.composer-toggle-smooth.composer-toggle-hidden {
		transition-delay: 0ms, 0ms, 0ms, var(--toggle-duration);
	}

	.toggle-slot {
		position: relative;
		width: 40px;
		height: 40px;
	}

	.toggle-child {
		position: absolute;
		inset: 0;
		transition:
			opacity 150ms cubic-bezier(0.4, 0, 0.2, 1),
			transform 150ms cubic-bezier(0.4, 0, 0.2, 1),
			visibility 0ms;
	}

	.toggle-child.toggle-hidden {
		visibility: hidden;
		opacity: 0;
		transform: scale(0.6);
		transition-delay: 0ms, 0ms, 150ms;
	}

	.ios-send {
		transition:
			opacity 250ms cubic-bezier(0.25, 0.1, 0.25, 1),
			transform 250ms cubic-bezier(0.25, 0.1, 0.25, 1),
			width 250ms cubic-bezier(0.25, 0.1, 0.25, 1),
			margin-inline-start 250ms cubic-bezier(0.25, 0.1, 0.25, 1);
	}

	.ios-send-hidden {
		opacity: 0;
		transform: scale(0.1);
	}

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
