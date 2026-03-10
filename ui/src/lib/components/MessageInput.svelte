<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiSend,
		mdiEmoticonHappyOutline,
		mdiPlus,
		mdiImage,
		mdiFile,
		mdiClose,
		mdiMicrophone,
		mdiStop,
	} from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { onMount, tick } from 'svelte';
	import { isIos } from '$lib/utils/environment';
	import {
		type Media,
		type PhotoItem,
		fileToDataUrl,
		formatFileSize,
	} from '$lib/types/media';
	import {
		startRecording,
		formatDuration,
		type AudioRecorderHandle,
	} from '$lib/utils/audio-recorder';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		value?: string;
		placeholder?: string;
		height: string;
		media?: Media | undefined;
		onSend?: () => void;
		onInput?: () => void;
		onEmojiClick?: () => void;
		onMediaChange?: (media: Media | undefined) => void;
	}

	let {
		value = $bindable(''),
		height = $bindable(''),
		placeholder = m.typeMessage(),
		media = undefined,
		onSend,
		onInput,
		onEmojiClick,
		onMediaChange,
	}: Props = $props();
	let div: HTMLDivElement;
	let showAttachMenu = $state(false);
	let photoFilePicker: HTMLInputElement;
	let fileFilePicker: HTMLInputElement;

	// Audio recording state
	let recording = $state(false);
	let recorderHandle: AudioRecorderHandle | null = null;
	let recordingElapsed = $state(0);
	let recordingTimer: ReturnType<typeof setInterval> | null = null;

	let hasContent = $derived(value.trim().length > 0 || !!media);

	async function onPhotosSelected(input: HTMLInputElement) {
		if (!input.files || input.files.length === 0) return;
		const photos: PhotoItem[] = [];
		for (let i = 0; i < input.files.length; i++) {
			const file = input.files[i];
			const dataUrl = await fileToDataUrl(file);
			photos.push({ dataUrl, file });
		}
		onMediaChange?.({ kind: 'photos', photos });
		input.value = '';
		showAttachMenu = false;
		await tick();
		updateHeight();
	}

	function onFilePickerSelected(input: HTMLInputElement) {
		if (!input.files || !input.files[0]) return;
		const file = input.files[0];
		onMediaChange?.({ kind: 'file', file, name: file.name, size: file.size });
		input.value = '';
		showAttachMenu = false;
		tick().then(updateHeight);
	}

	function removeMedia() {
		onMediaChange?.(undefined);
		tick().then(updateHeight);
	}

	function removePhoto(index: number) {
		if (!media || media.kind !== 'photos') return;
		const remaining = media.photos.filter((_, i) => i !== index);
		onMediaChange?.(remaining.length > 0 ? { kind: 'photos', photos: remaining } : undefined);
		tick().then(updateHeight);
	}

	function updateHeight() {
		if (div) height = `${div.scrollHeight}px`;
	}

	const theme = $derived(useTheme());

	let textarea: HTMLTextAreaElement;

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			triggerOnSend();
		}
	}

	function handleInput() {
		value = textarea.value;
		autoResize();
		onInput?.();
	}

	function autoResize() {
		if (textarea.scrollHeight > 100) return;
		textarea.style.height = 'auto';
		const textareaHeight = textarea.scrollHeight + 'px';
		textarea.style.height = textareaHeight;
		height = `${div.scrollHeight}px`;
	}

	function handleSendClick() {
		triggerOnSend();
	}

	function triggerOnSend() {
		if (hasContent) {
			onSend?.();
			textarea.style.height = 'auto';
			tick().then(updateHeight);
		}
	}

	async function toggleRecording() {
		if (recording) {
			await stopRecording();
		} else {
			await startRecordingAudio();
		}
	}

	async function startRecordingAudio() {
		try {
			recorderHandle = await startRecording();
			recording = true;
			recordingElapsed = 0;
			recordingTimer = setInterval(() => {
				if (recorderHandle) {
					recordingElapsed = Date.now() - recorderHandle.startTime;
				}
			}, 200);
		} catch (err) {
			if (err instanceof DOMException && (err.name === 'NotAllowedError' || err.name === 'NotFoundError')) {
				showToast(m.microphonePermissionDenied());
			} else {
				console.error('Failed to start recording:', err);
			}
		}
	}

	async function stopRecording() {
		const handle = recorderHandle;
		if (!handle) return;
		// Immediately reset state so the UI switches back and
		// a second tap can't call stop() again during transcoding.
		recorderHandle = null;
		recording = false;
		recordingElapsed = 0;
		if (recordingTimer) {
			clearInterval(recordingTimer);
			recordingTimer = null;
		}
		try {
			const result = await handle.stop();
			onMediaChange?.({
				kind: 'audio',
				dataUrl: result.dataUrl,
				mimeType: result.mimeType,
				durationMs: result.durationMs,
				size: result.size,
			});
			await tick();
			updateHeight();
		} catch (err) {
			console.error('Failed to stop recording:', err);
		}
	}

	function cancelRecording() {
		if (recorderHandle) {
			recorderHandle.cancel();
		}
		if (recordingTimer) {
			clearInterval(recordingTimer);
			recordingTimer = null;
		}
		recording = false;
		recorderHandle = null;
		recordingElapsed = 0;
	}

	onMount(() => {
		height = `${div.scrollHeight}px`;
	});
</script>

<div
	bind:this={div}
	class="message-input-bar m-2 pb-safe"
	class:bg-md-light-surface={theme === 'material'}
	class:dark:bg-md-dark-surface={theme === 'material'}
>
	<input
		type="file"
		accept="image/*,video/*"
		multiple
		bind:this={photoFilePicker}
		class="hidden"
		onchange={() => onPhotosSelected(photoFilePicker)}
	/>
	<input
		type="file"
		bind:this={fileFilePicker}
		class="hidden"
		onchange={() => onFilePickerSelected(fileFilePicker)}
	/>

	{#if media}
		<div class="media-preview">
			{#if media.kind === 'photos'}
				<div class="photo-preview-row">
					{#each media.photos as photo, i}
						<div class="photo-thumb-wrapper">
							<img src={photo.dataUrl} alt="" class="photo-thumb" />
							<button
								type="button"
								class="thumb-remove"
								onclick={() => removePhoto(i)}
								aria-label="Remove"
							>
								<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
							</button>
						</div>
					{/each}
				</div>
			{:else if media.kind === 'file'}
				<div class="file-preview">
					<wa-icon src={wrapPathInSvg(mdiFile)} class="file-preview-icon"></wa-icon>
					<div class="file-preview-info">
						<span class="file-preview-name">{media.name}</span>
						<span class="file-preview-size">{formatFileSize(media.size)}</span>
					</div>
					<button
						type="button"
						class="thumb-remove"
						onclick={removeMedia}
						aria-label="Remove"
					>
						<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
					</button>
				</div>
			{:else if media.kind === 'audio'}
				<div class="audio-preview">
					<audio src={media.dataUrl} controls class="audio-preview-player"></audio>
					<button
						type="button"
						class="thumb-remove"
						onclick={removeMedia}
						aria-label="Remove"
					>
						<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
					</button>
				</div>
			{/if}
		</div>
	{/if}

	{#if recording}
		<div
			class="row gap-2"
			style="align-items: flex-end; margin: 0 auto"
		>
			<button
				type="button"
				class="cancel-record-button"
				onclick={cancelRecording}
				aria-label="Cancel recording"
				style="align-self: flex-end; margin-bottom: 4px;"
			>
				<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
			</button>

			<div class="recording-indicator">
				<span class="recording-dot"></span>
				<span class="recording-time">{formatDuration(recordingElapsed)}</span>
			</div>

			<button
				type="button"
				class="stop-record-button"
				onclick={stopRecording}
				aria-label="Stop recording"
				data-testid="message-input-stop-record"
			>
				<wa-icon src={wrapPathInSvg(mdiStop)}></wa-icon>
			</button>
		</div>
	{:else}
		<div
			class="row gap-2"
			style="align-items: flex-end; margin: 0 auto"
		>
			<div
				class={theme === 'ios'
					? 'input-container bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'input-container bg-white dark:bg-gray-800'}
			>
				{#if onEmojiClick && !isIos}
					<button
						type="button"
						class="icon-button emoji-btn"
						onclick={onEmojiClick}
						aria-label="Emoji"
						data-testid="message-input-emoji"
					>
						<wa-icon src={wrapPathInSvg(mdiEmoticonHappyOutline)}></wa-icon>
					</button>
				{/if}

				<textarea
					class="message-textarea"
					data-testid="message-input-textarea"
					{placeholder}
					bind:value
					bind:this={textarea}
					rows="1"
					onkeydown={handleKeydown}
					oninput={handleInput}
				></textarea>
			</div>

			<button
				type="button"
				class="mic-button"
				onclick={toggleRecording}
				aria-label="Record voice message"
				data-testid="message-input-mic"
				style="align-self: flex-end; margin-bottom: 4px;"
			>
				<wa-icon src={wrapPathInSvg(mdiMicrophone)}></wa-icon>
			</button>

			<div class="relative" style="align-self: flex-end; margin-bottom: 4px;">
				<button
					type="button"
					class="attach-button"
					data-testid="message-input-attach"
					onclick={() => (showAttachMenu = !showAttachMenu)}
					aria-label="Attach"
				>
					<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
				</button>
				{#if showAttachMenu}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="fixed inset-0 z-10"
						onclick={() => (showAttachMenu = false)}
						onkeydown={() => {}}
					></div>
					<div class="attach-menu" data-testid="message-input-attach-menu">
						<button
							class="attach-menu-item"
							data-testid="message-input-attach-photos"
							onclick={() => {
								showAttachMenu = false;
								photoFilePicker.click();
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiImage)}></wa-icon>
							<span>{m.photosAndVideo()}</span>
						</button>
						<button
							class="attach-menu-item"
							data-testid="message-input-attach-file"
							onclick={() => {
								showAttachMenu = false;
								fileFilePicker.click();
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiFile)}></wa-icon>
							<span>{m.menuFile()}</span>
						</button>
					</div>
				{/if}
			</div>

			<button
				type="button"
				class="send-button"
				data-testid="message-input-send"
				class:active={hasContent}
				onclick={handleSendClick}
				disabled={!hasContent}
				aria-label="Send"
			>
				<wa-icon src={wrapPathInSvg(mdiSend)}></wa-icon>
			</button>
		</div>
	{/if}
</div>

<style>
	.input-container {
		flex: 1;
		display: flex;
		align-items: flex-end;
		min-width: 0;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		padding: 4px 4px 4px 6px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--k-theme-color, #3b82f6);
	}

	.icon-button {
		flex-shrink: 0;
		width: 36px;
		height: 36px;
		border: none;
		background: transparent;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.5;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
		padding: 0;
	}

	.icon-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.1);
	}

	.icon-button:active {
		background: rgba(128, 128, 128, 0.2);
	}

	.message-textarea {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		resize: none;
		font-size: 16px;
		line-height: 1.375;
		padding: 8px 8px;
		color: var(--k-text-color);
		font-family: inherit;
		min-height: 20px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}

	.send-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		margin-bottom: 4px;
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.4;
		transition:
			background-color 0.2s ease,
			opacity 0.2s ease,
			transform 0.1s ease;
	}

	.send-button:disabled {
		cursor: default;
	}

	.send-button.active {
		background: var(--k-theme-color, #3b82f6);
		color: white;
		opacity: 1;
	}

	.send-button.active:hover {
		filter: brightness(1.1);
	}

	.send-button.active:active {
		transform: scale(0.95);
	}

	/* Icon sizing */
	.icon-button :global(wa-icon),
	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.send-button :global(wa-icon) {
		margin-left: 2px; /* Optical centering for send arrow */
	}

	.attach-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.6;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}

	.attach-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.25);
	}

	.attach-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.mic-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.6;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}

	.mic-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.25);
	}

	.mic-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.attach-menu {
		position: absolute;
		bottom: calc(100% + 8px);
		right: 0;
		z-index: 20;
		min-width: 180px;
		border-radius: 12px;
		padding: 4px 0;
		background: var(--k-bars-bg-color);
		border: 1px solid var(--k-hairline-color);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
	}

	.attach-menu-item {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		padding: 10px 16px;
		border: none;
		background: transparent;
		cursor: pointer;
		font-size: 15px;
		color: var(--k-text-color);
		transition: background-color 0.1s ease;
	}

	.attach-menu-item:hover {
		background: rgba(128, 128, 128, 0.1);
	}

	.attach-menu-item :global(wa-icon) {
		width: 20px;
		height: 20px;
		opacity: 0.7;
	}

	/* Recording UI */
	.recording-indicator {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 12px;
		height: 44px;
		border-radius: 22px;
		background: rgba(128, 128, 128, 0.1);
	}

	.recording-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: #ef4444;
		animation: pulse-dot 1s ease-in-out infinite;
	}

	@keyframes pulse-dot {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
	}

	.recording-time {
		font-size: 15px;
		font-variant-numeric: tabular-nums;
		color: var(--k-text-color);
	}

	.cancel-record-button,
	.stop-record-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}

	.cancel-record-button {
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.6;
	}

	.cancel-record-button:hover {
		opacity: 0.8;
		background: rgba(128, 128, 128, 0.25);
	}

	.stop-record-button {
		background: #ef4444;
		color: white;
		opacity: 1;
		margin-bottom: 4px;
	}

	.stop-record-button:hover {
		filter: brightness(1.1);
	}

	.cancel-record-button :global(wa-icon),
	.stop-record-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	/* Audio preview */
	.audio-preview {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-radius: 10px;
		background: rgba(128, 128, 128, 0.1);
		position: relative;
	}

	.audio-preview-player {
		flex: 1;
		height: 36px;
		min-width: 0;
	}

	/* Media preview */
	.media-preview {
		padding: 8px 8px 0;
	}

	.photo-preview-row {
		display: flex;
		gap: 6px;
		overflow-x: auto;
		padding-bottom: 4px;
	}

	.photo-thumb-wrapper {
		position: relative;
		flex-shrink: 0;
	}

	.photo-thumb {
		width: 72px;
		height: 72px;
		object-fit: cover;
		border-radius: 8px;
	}

	.thumb-remove {
		position: absolute;
		top: -6px;
		right: -6px;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: none;
		background: rgba(0, 0, 0, 0.6);
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
	}

	.thumb-remove :global(wa-icon) {
		width: 14px;
		height: 14px;
	}

	.file-preview {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 12px;
		border-radius: 10px;
		background: rgba(128, 128, 128, 0.1);
		position: relative;
	}

	.file-preview :global(.file-preview-icon) {
		width: 28px;
		height: 28px;
		opacity: 0.6;
		flex-shrink: 0;
	}

	.file-preview-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.file-preview-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--k-text-color);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.file-preview-size {
		font-size: 12px;
		color: var(--k-text-color);
		opacity: 0.5;
	}
</style>
