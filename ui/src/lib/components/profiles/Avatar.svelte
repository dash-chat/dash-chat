<script module lang="ts">
	export const TEXT_AVATAR_TEXT_COLOR = '#831843';
</script>

<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiAccountQuestion } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TextAvatarData } from './text-avatar-data-url';
	import type { Snippet } from 'svelte';

	let {
		image,
		initials,
		alt,
		style,
		id,
		children,
	}: {
		image?: string | undefined;
		initials?: string | undefined;
		alt?: string | undefined;
		style?: string | undefined;
		id?: string | undefined;
		children?: Snippet | undefined;
	} = $props();

	const textAvatarData = $derived(TextAvatarData.deserialize(image));
	const avatarImage = $derived(
		image?.startsWith('data:image') ? image : undefined,
	);
	const avatarInitials = $derived(
		textAvatarData?.text || initials || undefined,
	);
	const avatarStyle = $derived.by(() => {
		if (!textAvatarData) {
			return style;
		}

		const textAvatarStyle = `background-color: ${textAvatarData.sanitizedHexColor()}; color: ${TEXT_AVATAR_TEXT_COLOR};`;
		return style ? `${style}; ${textAvatarStyle}` : textAvatarStyle;
	});
	const showPlaceholder = $derived(!avatarImage && !avatarInitials);
</script>

<wa-avatar
	{id}
	image={avatarImage}
	initials={avatarInitials}
	style={avatarStyle}
	{alt}
	shape="circle"
>
	{#if showPlaceholder}
		<wa-icon
			slot="icon"
			src={wrapPathInSvg(mdiAccountQuestion)}
			style="font-size: calc(var(--size, 3rem) * 0.5)"
		></wa-icon>
	{/if}
	{@render children?.()}
</wa-avatar>
