import {
	type DraftMedia,
	type DraftVoiceNote,
	MAX_MESSAGE_BYTES,
	type StagingError,
	UndecodablePhotoError,
	UnreadableFileError,
	inMemoryCopy,
	planStaging,
	prepareOutgoingPhoto,
} from '$lib/utils/media';
import type { OutgoingPhoto } from 'dash-chat-stores';

export interface StagingOutcome {
	error?: StagingError;
	undecodablePhotos: number;
	unreadablePhotos: number;
}

const NOTHING_LEFT_OUT: StagingOutcome = {
	undecodablePhotos: 0,
	unreadablePhotos: 0,
};

/** Owns the composer's staged attachments and is the only writer of the
 * draft, so a photo is in it only once it has been prepared for sending. */
export class MediaDraft {
	// Raw: the draft is only ever replaced whole, and its prepared photos go
	// to the backend as they are.
	#media = $state.raw<DraftMedia | undefined>();
	#pendingBatches = $state(0);
	#pendingPhotos = $state(0);
	#queue: Promise<void> = Promise.resolve();
	// Bumped by whatever discards the draft, so a batch still being staged
	// stops, and stops counting as pending, instead of bringing the draft back.
	#generation = 0;

	/** @param onStagingStarted called when a batch starts adding attachments,
	 * before its photos are prepared. */
	constructor(private onStagingStarted: () => void) {}

	get media(): DraftMedia | undefined {
		return this.#media;
	}

	get busy(): boolean {
		return this.#pendingBatches > 0;
	}

	/** Photos of the batch being staged that are still to be prepared. */
	get pendingPhotos(): number {
		return this.#pendingPhotos;
	}

	/** Stage what the mixing rules allow of `files`, after any batch already
	 * being staged. Photos enter the draft one by one as they are prepared. */
	add(files: File[]): Promise<StagingOutcome> {
		const generation = this.#generation;
		this.#pendingBatches++;
		const outcome = this.#queue
			.then(() => this.#stage(files, generation))
			.finally(() => {
				if (generation === this.#generation) this.#pendingBatches--;
			});
		this.#queue = outcome.then(
			() => {},
			() => {},
		);
		return outcome;
	}

	removePhoto(index: number) {
		if (this.#media?.kind !== 'photos') return;
		this.#setPhotos(this.#media.items.filter((_, i) => i !== index));
	}

	clear() {
		this.#generation++;
		this.#media = undefined;
		this.#pendingPhotos = 0;
		this.#pendingBatches = 0;
	}

	setVoiceNote(voice: DraftVoiceNote) {
		this.clear();
		this.#media = { kind: 'voice_note', voice };
	}

	/** Drop what `sent` held, keeping photos staged while it was being sent. */
	discardSent(sent: DraftMedia) {
		if (this.#media?.kind === 'photos' && sent.kind === 'photos') {
			this.#setPhotos(this.#media.items.filter(p => !sent.items.includes(p)));
		} else if (this.#media === sent) {
			this.#media = undefined;
		}
	}

	async #stage(files: File[], generation: number): Promise<StagingOutcome> {
		if (generation !== this.#generation) return NOTHING_LEFT_OUT;
		const plan = planStaging(this.#media, files);
		if (plan.file) return this.#stageFile(plan.file, generation);
		if (plan.photos.length === 0) {
			return { ...NOTHING_LEFT_OUT, error: plan.error };
		}
		this.#pendingPhotos = plan.photos.length;
		this.onStagingStarted();
		const leftOut = await this.#stagePhotos(plan.photos, generation);
		if (generation !== this.#generation) return NOTHING_LEFT_OUT;
		return { ...leftOut, error: plan.error };
	}

	async #stageFile(file: File, generation: number): Promise<StagingOutcome> {
		this.onStagingStarted();
		// One over the cap is only ever rejected at send, so it isn't read.
		const staged =
			file.size > MAX_MESSAGE_BYTES
				? file
				: await inMemoryCopyOrUnreadable(file);
		if (generation !== this.#generation) return NOTHING_LEFT_OUT;
		if (staged === 'unreadable') {
			return { ...NOTHING_LEFT_OUT, error: 'unreadableFile' };
		}
		this.#media = { kind: 'file', file: staged };
		return NOTHING_LEFT_OUT;
	}

	/** Returns how many of `files` were left out, by reason. */
	async #stagePhotos(
		files: File[],
		generation: number,
	): Promise<StagingOutcome> {
		const leftOut = { ...NOTHING_LEFT_OUT };
		try {
			// One at a time: decoding a batch of full-size photos at once can
			// exhaust the Android webview's memory.
			for (const file of files) {
				const photo = await prepareOrReasonLeftOut(file);
				if (generation !== this.#generation) return NOTHING_LEFT_OUT;
				this.#pendingPhotos--;
				if (photo === 'undecodable') {
					leftOut.undecodablePhotos++;
				} else if (photo === 'unreadable') {
					leftOut.unreadablePhotos++;
				} else {
					this.#appendPhoto(photo);
				}
			}
		} finally {
			if (generation === this.#generation) this.#pendingPhotos = 0;
		}
		return leftOut;
	}

	#appendPhoto(photo: OutgoingPhoto) {
		const staged = this.#media?.kind === 'photos' ? this.#media.items : [];
		this.#media = { kind: 'photos', items: [...staged, photo] };
	}

	#setPhotos(photos: OutgoingPhoto[]) {
		this.#media =
			photos.length > 0 ? { kind: 'photos', items: photos } : undefined;
	}
}

async function prepareOrReasonLeftOut(
	file: File,
): Promise<OutgoingPhoto | 'undecodable' | 'unreadable'> {
	try {
		return await prepareOutgoingPhoto(file);
	} catch (e) {
		if (e instanceof UndecodablePhotoError) return 'undecodable';
		if (e instanceof UnreadableFileError) return 'unreadable';
		throw e;
	}
}

async function inMemoryCopyOrUnreadable(
	file: File,
): Promise<File | 'unreadable'> {
	try {
		return await inMemoryCopy(file);
	} catch (e) {
		if (e instanceof UnreadableFileError) return 'unreadable';
		throw e;
	}
}
