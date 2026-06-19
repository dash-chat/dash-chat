<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { useTheme } from 'konsta/svelte';
	import { mdiMicrophone } from '@mdi/js';
	import { showToast } from '$lib/utils/toasts';
	import type { DraftVoiceNote } from '$lib/utils/media';
	import IconButton from '$lib/components/IconButton.svelte';
	import { VoiceRecorder } from './useVoiceRecorder.svelte';
	import VoiceRecordingOverlay from './VoiceRecordingOverlay.svelte';
	import VoiceLockedBar from './VoiceLockedBar.svelte';

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

	const showLockedBar = $derived(
		recorder.phase === 'locked' || recorder.phase === 'encoding',
	);

	recorder.onMaxDuration = () => void stopAndSend();

	async function stopAndSend() {
		const draft = await recorder.stop();
		if (draft) onRecorded(draft);
	}

	async function onPointerDown(event: PointerEvent) {
		event.preventDefault();
		const el = event.currentTarget as HTMLElement;
		el.setPointerCapture(event.pointerId);
		startX = event.clientX;
		startY = event.clientY;
		willCancel = false;
		isRtl = getComputedStyle(el).direction === 'rtl';
		await recorder.start();
		if (recorder.phase === 'denied') {
			showToast(m.voiceMicDenied(), 'error');
			recorder.phase = 'idle';
			return;
		}
		if (recorder.phase !== 'recording') return;
		// A mouse can't comfortably press-and-hold, so a click records hands-free.
		if (event.pointerType === 'mouse') recorder.lock();
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
		if (recorder.phase !== 'recording') return;
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

{#if recorder.phase === 'recording'}
	<div
		class="voice-layer pointer-events-none {theme === 'ios'
			? 'bg-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass'
			: 'bg-white dark:bg-gray-800'}"
	>
		<VoiceRecordingOverlay elapsedMs={recorder.elapsedMs} {drag} />
	</div>
{:else if showLockedBar}
	<div
		class="voice-layer {theme === 'ios'
			? 'bg-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass'
			: 'bg-white dark:bg-gray-800'}"
	>
		<VoiceLockedBar
			elapsedMs={recorder.elapsedMs}
			onCancel={() => void recorder.cancel()}
			onSend={() => void stopAndSend()}
		/>
	</div>
{/if}

<IconButton
	icon={mdiMicrophone}
	label={m.voiceRecordHint()}
	testid="message-input-voice-record"
	class="h-[42px] w-[42px] shrink-0 touch-none"
	onpointerdown={onPointerDown}
	onpointermove={onPointerMove}
	onpointerup={onPointerUp}
	onpointercancel={() => recorder.cancel()}
/>

<style>
	.voice-layer {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		border-radius: 22px;
	}
</style>
