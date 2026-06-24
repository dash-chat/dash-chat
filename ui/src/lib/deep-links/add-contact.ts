import { goto } from '$app/navigation';
import { showToast } from '$lib/utils/toasts';

export const path = '/add-contact/{{code}}';

export function handle({ code }: Record<string, string>) {
	// TODO: This is temporary until another PR actually uses the code to add
	//       the contact (hence why it doesn't use paraglide messages)
	goto('/new-message/add-contact').then(() =>
		showToast(`Got a deep link with code: ${code}`),
	);
}
