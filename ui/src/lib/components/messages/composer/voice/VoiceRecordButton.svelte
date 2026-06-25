<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { useTheme } from 'konsta/svelte';
	import { mdiMicrophone, mdiLockOutline, mdiChevronUp } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { showToast } from '$lib/utils/toasts';
	import { isMobile } from '$lib/utils/environment';
	import type { DraftVoiceNote } from '$lib/utils/media';
	import IconButton from '$lib/components/IconButton.svelte';
	import { VoiceRecorder } from './useVoiceRecorder.svelte';
	import VoiceRecordingOverlay from './VoiceRecordingOverlay.svelte';
	import VoiceLockedBar from './VoiceLockedBar.svelte';
	import VoiceDesktopBar from './VoiceDesktopBar.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';

	export interface DragState {
		active: boolean;
		cancelProgress: number;
		lockProgress: number;
	}

	interface Props {
		/** Called with the finished recording when it should be sent. */
		onRecorded: (draft: DraftVoiceNote) => void;
	}

	let { onRecorded }: Props = $props();

	const theme = $derived(useTheme());

	/** Pixels the pointer must travel toward the inline-start to cancel. */
	const CANCEL_THRESHOLD = 120;
	/** Pixels the pointer must travel upward to lock hands-free recording. */
	const LOCK_THRESHOLD = 80;
	/** A press shorter than this is treated as a tap, not a recording. */
	const MIN_DURATION_MS = 600;

	const recorder = new VoiceRecorder();
	const idle: DragState = { active: false, cancelProgress: 0, lockProgress: 0 };
	let drag: DragState = $state(idle);

	let startX = 0;
	let startY = 0;
	let isRtl = false;
	let willCancel = false;
	// The pointer can be released while `recorder.start()` is still awaiting mic
	// permission/native start; remember that so we finish the hold once recording
	// actually begins instead of leaving it stuck recording.
	let releasedWhileStarting = false;

	const showLockedBar = $derived(
		recorder.phase === 'locked' || recorder.phase === 'encoding',
	);

	// The press-and-hold visuals (red mic, slide-up-to-lock pill) only make sense
	// mid-hold on touch; desktop click-records straight into the locked bar. Show
	// them already during `requesting` so the overlay appears the instant the user
	// presses, rather than after the native recorder has finished starting up.
	const recordingHoldMobile = $derived(
		(recorder.phase === 'recording' || recorder.phase === 'requesting') &&
			isMobile,
	);

	// While the locked/desktop bar replaces the input row, the mic button must be
	// hidden: it overlaps the bar's send button and, painting later, would steal
	// its taps.
	const barReplacesButton = $derived(
		!recordingHoldMobile && (showLockedBar || recorder.isActive),
	);

	recorder.onMaxDuration = () => void stopAndSend();

	async function stopAndSend(): Promise<boolean> {
		let draft: DraftVoiceNote | undefined;
		try {
			draft = await recorder.stop();
		} catch (e) {
			console.error('Failed to finish voice recording', e);
			showToast(m.voiceRecordFailed(), 'error');
			return false;
		}
		if (draft) onRecorded(draft);
		return !!draft;
	}

	async function onPointerDown(event: PointerEvent) {
		event.preventDefault();
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		startX = event.clientX;
		startY = event.clientY;
		willCancel = false;
		releasedWhileStarting = false;
		isRtl = getComputedStyle(el).direction === 'rtl';
		await recorder.start();
		if (recorder.phase === 'denied') {
			showToast(m.voiceMicDenied(), 'error');
			recorder.phase = 'idle';
			return;
		}
		if (recorder.phase !== 'recording') return;
		// A mouse can't comfortably press-and-hold, so a click records hands-free.
		if (event.pointerType === 'mouse') {
			recorder.lock();
			return;
		}
		if (releasedWhileStarting) await finishHold();
	}

	function onPointerMove(event: PointerEvent) {
		if (recorder.phase !== 'recording') return;
		const inlineTowardStart = isRtl
			? event.clientX - startX
			: startX - event.clientX;
		const up = startY - event.clientY;
		willCancel = inlineTowardStart >= CANCEL_THRESHOLD;
		drag = {
			active: true,
			cancelProgress: clamp01(inlineTowardStart / CANCEL_THRESHOLD),
			lockProgress: clamp01(up / LOCK_THRESHOLD),
		};
		if (up >= LOCK_THRESHOLD) {
			recorder.lock();
			drag = idle;
		}
	}

	async function onPointerUp() {
		drag = idle;
		// Released before the async start finished: defer the finish to onPointerDown.
		if (recorder.phase === 'requesting') {
			releasedWhileStarting = true;
			return;
		}
		if (recorder.phase !== 'recording') return;
		await finishHold();
	}

	async function onPointerCancel() {
		drag = idle;
		if (recorder.phase === 'requesting') {
			willCancel = true;
			releasedWhileStarting = true;
			return;
		}
		await recorder.cancel();
	}

	async function finishHold() {
		if (willCancel || recorder.elapsedMs < MIN_DURATION_MS) {
			await recorder.cancel();
			if (!willCancel) showToast(m.voiceRecordHint(), 'default');
			return;
		}
		await stopAndSend();
	}

	function clamp01(value: number): number {
		return Math.max(0, Math.min(1, value));
	}

	// Free the mic if we leave the chat mid-recording.
	onDestroy(() => void recorder.cancel());
</script>

{#if recordingHoldMobile}
	<div
		class="voice-layer has-end-button pointer-events-none {theme === 'ios'
			? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
			: 'bg-white dark:bg-gray-800'}"
	>
		<VoiceRecordingOverlay elapsedMs={recorder.elapsedMs} {drag} />
	</div>
{:else if showLockedBar || recorder.isActive}
	{#if isMobile}
		<div
			class="voice-layer has-end-button {theme === 'ios'
				? 'bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
				: 'bg-white dark:bg-gray-800'}"
		>
			<VoiceLockedBar
				elapsedMs={recorder.elapsedMs}
				onCancel={() => void recorder.cancel()}
			/>
		</div>
	{:else}
		<div class="voice-layer voice-layer-flush bg-page-surface">
			<VoiceDesktopBar
				elapsedMs={recorder.elapsedMs}
				onCancel={() => void recorder.cancel()}
				onSend={stopAndSend}
			/>
		</div>
	{/if}
{/if}

{#if barReplacesButton && isMobile}
	<div class="relative z-30 shrink-0">
		<SendButton disabled={false} onSend={stopAndSend} testid="voice-send" />
	</div>
{:else if !barReplacesButton}
	<div class="relative shrink-0 {recordingHoldMobile ? 'z-30' : ''}">
		{#if recordingHoldMobile}
			<div
				class="lock-pill pointer-events-none absolute bottom-full start-1/2 mb-2 flex flex-col items-center gap-1.5 rounded-full bg-gray-100 px-1.5 py-2.5 dark:bg-gray-700"
				style="transform: translate(-50%, {-8 * drag.lockProgress}px)"
			>
				<wa-icon
					class="lock-icon"
					src={wrapPathInSvg(mdiLockOutline)}
					style="opacity: {0.55 + 0.45 * drag.lockProgress}"
				></wa-icon>
				<wa-icon class="chevron-up" src={wrapPathInSvg(mdiChevronUp)}></wa-icon>
			</div>
		{/if}

		<IconButton
			icon={mdiMicrophone}
			label={m.voiceRecordHint()}
			testid="message-input-voice-record"
			iconClass={recordingHoldMobile ? 'text-2xl text-white' : 'text-2xl'}
			class="h-[42px] w-[42px] shrink-0 touch-none {recordingHoldMobile
				? '!bg-red-500 !opacity-100'
				: ''}"
			onpointerdown={onPointerDown}
			onpointermove={onPointerMove}
			onpointerup={onPointerUp}
			onpointercancel={onPointerCancel}
		/>
	</div>
{/if}

<style>
	.voice-layer {
		position: absolute;
		inset-block: 0;
		inset-inline: 0;
		display: flex;
		align-items: center;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		/* The composer's emoji/attach/mic buttons (Konsta `Button`) sit at z-index 10;
		   the overlay must paint above them so they don't bleed through. */
		z-index: 20;
	}
	/* Leave the trailing slot free so the action button (mic / send) sits outside
	   the bordered pill, mirroring the message input's send button. */
	.voice-layer.has-end-button {
		inset-inline-end: calc(42px + 0.5rem);
	}
	/* On desktop the bar spans the full row and lays out its own inner pill plus
	   the Cancel/Send buttons, so the overlay itself must not look like a pill —
	   it just paints the composer surface to hide the input row underneath. */
	.voice-layer.voice-layer-flush {
		border: none;
		border-radius: 0;
	}
	.lock-pill :global(wa-icon) {
		width: 18px;
		height: 18px;
		color: var(--k-text-color);
	}
	.lock-pill .chevron-up {
		animation: nudge-up 1s ease-in-out infinite;
	}
	@keyframes nudge-up {
		0%,
		100% {
			transform: translateY(0);
			opacity: 0.5;
		}
		50% {
			transform: translateY(-3px);
			opacity: 0.9;
		}
	}
</style>
