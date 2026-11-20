import { invokeAfterSetup } from 'dash-chat-stores';

/** The thrown value, split into the parts Sentry needs to title and group it. */
interface ReportedError {
	name: string;
	message: string;
	stack?: string;
}

interface ErrorReport {
	message: string;
	error?: ReportedError;
}

export type FeedbackReason =
	| 'bug'
	| 'feature'
	| 'question'
	| 'feedback'
	| 'other';

interface Feedback {
	reason: FeedbackReason;
	message: string;
	screenshots: File[];
	includeLogs: boolean;
}

/** Whether a report reached Sentry or is waiting on disk for a connection. */
export type SendOutcome = 'sent' | 'queued';

export async function sendErrorReport(
	report: ErrorReport,
): Promise<SendOutcome> {
	return invokeAfterSetup('plugin:sentry-reporting|send_error_report', {
		message: report.message,
		error: report.error,
	});
}

/** Mirrors `Screenshot` in the plugin's feedback.rs. */
interface Screenshot {
	contentType: string;
	base64: string;
}

export async function sendFeedback(feedback: Feedback): Promise<SendOutcome> {
	const screenshots: Screenshot[] = [];
	for (const file of feedback.screenshots) {
		screenshots.push(await describeScreenshot(file));
	}
	return invokeAfterSetup('plugin:sentry-reporting|send_feedback', {
		feedback: {
			reason: feedback.reason,
			message: feedback.message,
			screenshots,
			includeLogs: feedback.includeLogs,
		},
	});
}

/** Bytes rather than a path, so the image never has to be staged to disk; no
 * name, since the plugin numbers the attachments so none leaves the device. */
async function describeScreenshot(file: File): Promise<Screenshot> {
	return {
		contentType: file.type,
		base64: await base64Of(file),
	};
}

function base64Of(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => {
			const dataUrl = reader.result as string;
			resolve(dataUrl.slice(dataUrl.indexOf(',') + 1));
		};
		reader.onerror = () => reject(reader.error);
		reader.readAsDataURL(file);
	});
}

export async function hasPendingCrashReport(): Promise<boolean> {
	return invokeAfterSetup('plugin:sentry-reporting|pending_crash_report');
}

export async function sendPendingCrashReport(): Promise<SendOutcome> {
	return invokeAfterSetup('plugin:sentry-reporting|send_pending_crash_report');
}

export async function discardPendingCrashReport(): Promise<void> {
	return invokeAfterSetup(
		'plugin:sentry-reporting|discard_pending_crash_report',
	);
}

export function describeError(error: unknown): ReportedError | undefined {
	if (error === undefined || error === null) return undefined;
	if (error instanceof Error) {
		return { name: error.name, message: error.message, stack: error.stack };
	}
	if (typeof error === 'string') return { name: 'Error', message: error };
	try {
		return { name: 'Error', message: JSON.stringify(error) };
	} catch {
		return { name: 'Error', message: String(error) };
	}
}
