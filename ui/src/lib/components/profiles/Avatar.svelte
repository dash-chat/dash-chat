<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { TextAvatarData } from './text-avatar-data-url';
	import type { Snippet } from 'svelte';

	const TEXT_AVATAR_TEXT_COLOR = '#831843';

	let {
		image,
		initials,
		alt,
		style,
		id,
		children,
	}: {
		image: string | undefined;
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
	const avatarInitials = $derived(textAvatarData?.text ?? initials);
	const avatarStyle = $derived.by(() => {
		if (!textAvatarData) {
			return style;
		}

		const textAvatarStyle = `background-color: ${textAvatarData.sanitizedHexColor()}; color: ${TEXT_AVATAR_TEXT_COLOR};`;
		return style ? `${style}; ${textAvatarStyle}` : textAvatarStyle;
	});
</script>

<wa-avatar
	{id}
	image={avatarImage}
	initials={avatarInitials}
	style={avatarStyle}
	{alt}
	shape="circle">{@render children?.()}</wa-avatar
>
