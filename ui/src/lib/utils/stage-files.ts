import { m } from '$lib/paraglide/messages.js';
import {
	type DraftMedia,
	type IngestError,
	ingestFiles,
} from '$lib/types/media';
import { showToast } from '$lib/utils/toasts';

const errorMessages: Record<IngestError, () => string> = {
	tooMany: () => m.errorTooManyAttachments(),
	filesWithPhotos: () => m.errorFilesWithPhotos(),
	oneFileAtATime: () => m.errorOneFileAtATime(),
};

/**
 * Add files to the composer draft, toasting if a Signal mixing rule was
 * violated. Returns the new draft (possibly unchanged).
 */
export function stageFiles(
	current: DraftMedia | undefined,
	files: FileList | File[],
): DraftMedia | undefined {
	const { media, error } = ingestFiles(current, Array.from(files));
	if (error) showToast(errorMessages[error](), 'error');
	return media;
}
