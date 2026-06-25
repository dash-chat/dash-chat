import { goto } from '$app/navigation';

export const path = '/add-contact/{{code}}';

export function handle({ code }: Record<string, string>) {
	goto(`/new-message/add-contact?code=${encodeURIComponent(code)}`);
}
