import { goto } from '$app/navigation';

import { buildHttpsDeepLinkUrl, extractDeepLinkParams } from './helpers';

export const path = '/add-contact/{{code}}';

export function toDeepLink(code: string): string | null {
	return buildHttpsDeepLinkUrl(path, { code });
}

export function extractCodeFromDeepLink(input: string): string | null {
	let params = extractDeepLinkParams(input, path);
	return params?.code ?? null;
}

export function handle({ code }: Record<string, string>) {
	goto(`/new-message/add-contact?code=${encodeURIComponent(code)}`);
}
