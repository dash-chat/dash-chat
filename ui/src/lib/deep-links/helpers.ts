export const HTTPS_DEEP_LINK_BASE_URL = 'https://dashchat.org';
export const SCHEME_DEEP_LINK_BASE_URL = 'dash-chat://';

function escapeRegex(s: string): string {
	return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function extractPathFromDeepLink(url: string): string | null {
	if (url.startsWith(HTTPS_DEEP_LINK_BASE_URL))
		return url.slice(HTTPS_DEEP_LINK_BASE_URL.length);
	if (url.startsWith(SCHEME_DEEP_LINK_BASE_URL))
		return '/' + url.slice(SCHEME_DEEP_LINK_BASE_URL.length);
	return null;
}

export function extractDeepLinkParams(
	url: string,
	path: string,
): Record<string, string> | null {
	const urlPath = extractPathFromDeepLink(url);
	if (urlPath === null) return null;
	const names: string[] = [];
	const pathPattern = path
		.split(/\{\{(\w+)\}\}/g)
		.map((chunk, i) => {
			if (i % 2 === 0) return escapeRegex(chunk);
			names.push(chunk);
			return '([^/?#]+)';
		})
		.join('');
	const match = urlPath.match(new RegExp(`^${pathPattern}(?:[?#].*)?$`));
	if (!match) return null;
	return Object.fromEntries(
		names.map((name, i) => {
			try {
				return [name, decodeURIComponent(match[i + 1])];
			} catch {
				return [name, match[i + 1]];
			}
		}),
	);
}

export function buildHttpsDeepLinkUrl(
	path: string,
	params: Record<string, string>,
): string | null {
	try {
		const urlPath = path.replace(/\{\{(\w+)\}\}/g, (_, name) => {
			if (!(name in params)) throw new Error(`Missing parameter: ${name}`);
			return encodeURIComponent(params[name]);
		});
		return HTTPS_DEEP_LINK_BASE_URL + urlPath;
	} catch (e) {
		console.error('[deep-link] failed to build URL:', e);
		return null;
	}
}
