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

/** Whether the build has a Sentry DSN, so the UI can hide the report action. */
export const errorReportingEnabled: Promise<boolean> =
	invokeAfterSetup<boolean>('plugin:sentry-reporting|is_enabled').catch(
		() => false,
	);

export async function sendErrorReport(report: ErrorReport): Promise<void> {
	return invokeAfterSetup('plugin:sentry-reporting|send_error_report', {
		message: report.message,
		error: report.error,
	});
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
