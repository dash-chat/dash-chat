import { blobUrl } from '$lib/utils/media';
import type { VoiceNote } from 'dash-chat-stores';

// Only one voice note plays at a time: starting a player pauses whichever was
// already playing, matching Signal.
let playing: VoicePlayer | undefined;

/**
 * Owns the playback of a single voice note: the one `<audio>` element, its
 * play/pause/seek logic, lazy byte loading, and the reactive state
 * (`paused`/`currentTime`/`loading`) that the play button and waveform both
 * render from. Creating one instance per voice note and sharing it keeps the
 * two views driving the same audio rather than each tracking its own.
 */
export class VoicePlayer {
	paused = $state(true);
	currentTime = $state(0);
	/** True while the bytes for the first play are being fetched. */
	loading = $state(false);

	#audio: HTMLAudioElement | undefined;
	#objectUrl: string | undefined;
	#loaded = false;
	#rafId: number | undefined;
	readonly #voice: VoiceNote;
	readonly #onError: (() => void) | undefined;

	constructor(voice: VoiceNote, onError?: () => void) {
		this.#voice = voice;
		this.#onError = onError;
	}

	/** The real playback length: prefer the loaded element's own duration over
	 * the recorded metadata, which can overshoot and leave the played region
	 * short of the end. Falls back to the metadata before the audio loads. */
	get durationSec(): number {
		const d = this.#audio?.duration;
		return d && isFinite(d) && d > 0 ? d : this.#voice.duration_ms / 1000;
	}

	/** Bind the DOM `<audio>` element and subscribe to its lifecycle. Returns a
	 * teardown to run on unmount (stops ticking, revokes the object URL). */
	attach(audio: HTMLAudioElement): () => void {
		this.#audio = audio;
		const onPlay = () => {
			if (playing && playing !== this) playing.#audio?.pause();
			playing = this;
			this.paused = false;
			this.#startTicking();
		};
		const onPause = () => {
			if (playing === this) playing = undefined;
			this.paused = true;
			this.#stopTicking();
			this.#sync();
		};
		const onEnded = () => {
			if (playing === this) playing = undefined;
			this.paused = true;
			this.#stopTicking();
			audio.currentTime = 0;
			this.#sync();
		};
		audio.addEventListener('play', onPlay);
		audio.addEventListener('pause', onPause);
		audio.addEventListener('timeupdate', this.#sync);
		audio.addEventListener('ended', onEnded);
		return () => {
			audio.removeEventListener('play', onPlay);
			audio.removeEventListener('pause', onPause);
			audio.removeEventListener('timeupdate', this.#sync);
			audio.removeEventListener('ended', onEnded);
			this.#stopTicking();
			if (playing === this) playing = undefined;
			if (this.#objectUrl) URL.revokeObjectURL(this.#objectUrl);
		};
	}

	async toggle(): Promise<void> {
		const audio = this.#audio;
		if (!audio || this.loading) return;
		if (!audio.paused) {
			audio.pause();
			return;
		}
		if (!(await this.#ensureLoaded())) return;
		try {
			await audio.play();
		} catch {
			this.#onError?.();
		}
	}

	async seekTo(fraction: number): Promise<void> {
		if (!(await this.#ensureLoaded()) || !this.#audio) return;
		this.#audio.currentTime =
			Math.max(0, Math.min(1, fraction)) * this.durationSec;
		this.#sync();
	}

	async seekBy(deltaSec: number): Promise<void> {
		if (!(await this.#ensureLoaded()) || !this.#audio) return;
		this.#audio.currentTime = Math.max(
			0,
			Math.min(this.durationSec, this.#audio.currentTime + deltaSec),
		);
		this.#sync();
	}

	// WebKitGTK's `<audio>` can't load our custom `irohblob://` scheme (its media
	// pipeline bypasses the webview's scheme handler), so the bytes are fetched
	// lazily on first play and set as an object URL.
	async #ensureLoaded(): Promise<boolean> {
		if (this.#loaded) return true;
		this.loading = true;
		try {
			const source = await this.#fetchAudio();
			if (!source) {
				this.#onError?.();
				return false;
			}
			if (!this.#audio) return false;
			this.#objectUrl = URL.createObjectURL(
				new Blob([source.data as BlobPart], { type: source.mimeType }),
			);
			this.#audio.src = this.#objectUrl;
			this.#loaded = true;
			return true;
		} finally {
			this.loading = false;
		}
	}

	/** Fetch the voice note's bytes for playback; undefined on failure. */
	async #fetchAudio(): Promise<
		{ data: Uint8Array; mimeType: string } | undefined
	> {
		try {
			const res = await fetch(blobUrl(this.#voice.hash));
			if (!res.ok) return undefined;
			return {
				data: new Uint8Array(await res.arrayBuffer()),
				mimeType: this.#voice.mime_type,
			};
		} catch {
			return undefined;
		}
	}

	#sync = (): void => {
		if (this.#audio) this.currentTime = this.#audio.currentTime;
	};

	// The `<audio>` element's `timeupdate` event is throttled (~4/sec), so the
	// played region would visibly lag. Repaint each frame while playing instead,
	// matching wavesurfer's own 60fps progress animation.
	#tick = (): void => {
		this.#sync();
		if (this.#audio && !this.#audio.paused)
			this.#rafId = requestAnimationFrame(this.#tick);
		else this.#rafId = undefined;
	};

	#startTicking(): void {
		if (this.#rafId === undefined)
			this.#rafId = requestAnimationFrame(this.#tick);
	}

	#stopTicking(): void {
		if (this.#rafId !== undefined) {
			cancelAnimationFrame(this.#rafId);
			this.#rafId = undefined;
		}
	}
}
