import { goto } from '$app/navigation';
import { matchesDeepLinkPath } from '$lib/utils/deep-links';

export const path = '/add-contact/{{code}}';

const BASE_URL = 'https://dashchat.org';

export function generate(code: string): string {
	return `${BASE_URL}/add-contact/${encodeURIComponent(code)}`;
}

export function extract(value: string): string | null {
	return matchesDeepLinkPath(value.trim(), path)?.code ?? null;
}

export function handle({ code }: Record<string, string>) {
	goto(`/new-message/add-contact?code=${encodeURIComponent(code)}`);
}
