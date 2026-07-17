import { goto } from '$app/navigation';

import { HTTPS_DEEP_LINK_BASE_URL } from './constants';

export const path = '/add-contact/{{code}}';

const deepLinkPrefix = HTTPS_DEEP_LINK_BASE_URL + path.replace('{{code}}', '');

export function toDeepLink(code: string): string {
	return deepLinkPrefix + code;
}

export function extractCodeFromDeepLink(input: string): string {
	if (input.startsWith(deepLinkPrefix)) {
		return input.slice(deepLinkPrefix.length);
	}
	return input;
}

export function handle({ code }: Record<string, string>) {
	goto(`/new-message/add-contact?code=${encodeURIComponent(code)}`);
}
