export const TOAST_TTL_MS = 3000;

export type ToastVariant = 'default' | 'error';

export interface ToastEvent {
	message: string;
	variant?: ToastVariant;
}

export function showToast(message: string, variant: ToastVariant = 'default') {
	window.dispatchEvent(
		new CustomEvent<ToastEvent>('app:toast', {
			detail: { message, variant },
		}),
	);
}
