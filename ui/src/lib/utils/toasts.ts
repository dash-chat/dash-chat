export const TOAST_TTL_MS = 3000;

export type ToastVariant = 'default' | 'error' | 'unexpected';

export interface ToastEvent {
	message: string;
	variant?: ToastVariant;
	error?: unknown;
}

export function showToast(message: string, variant: ToastVariant = 'default', error?: unknown) {
	window.dispatchEvent(
		new CustomEvent<ToastEvent>('app:toast', {
			detail: { message, variant, error },
		}),
	);
}
