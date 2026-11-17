<script module lang="ts">
	export const TEXT_AVATAR_TEXT_COLOR = '#831843';
</script>

<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TextAvatarData } from './text-avatar-data-url';

	let {
		image,
		initials,
		fallbackIcon,
		alt,
		size,
		style,
		id,
		testId,
	}: {
		image?: string | undefined;
		initials?: string | undefined;
		fallbackIcon?: string | undefined;
		alt?: string | undefined;
		size?: number | string | undefined;
		style?: string | undefined;
		id?: string | undefined;
		testId?: string | undefined;
	} = $props();

	const textAvatarData = $derived(TextAvatarData.deserialize(image));
	const avatarImage = $derived(
		image?.startsWith('data:image') ? image : undefined,
	);
	const avatarInitials = $derived(
		textAvatarData?.text || initials || undefined,
	);
	const sizeValue = $derived(
		size !== undefined
			? typeof size === 'number'
				? `${size}px`
				: size
			: undefined,
	);
	const baseStyle = $derived(
		sizeValue !== undefined
			? style
				? `--size: ${sizeValue}; ${style}`
				: `--size: ${sizeValue};`
			: style,
	);
	const avatarStyle = $derived.by(() => {
		if (!textAvatarData) {
			return baseStyle;
		}

		const textAvatarStyle = `background-color: ${textAvatarData.sanitizedHexColor()}; color: ${TEXT_AVATAR_TEXT_COLOR};`;
		return baseStyle ? `${baseStyle}; ${textAvatarStyle}` : textAvatarStyle;
	});
</script>

<span class="inline-block" data-testid={testId}>
	<wa-avatar
		{id}
		image={avatarImage}
		initials={avatarInitials}
		style={avatarStyle}
		{alt}
		shape="circle"
	>
		{#if fallbackIcon}
			<wa-icon
				slot="icon"
				src={wrapPathInSvg(fallbackIcon)}
				style="font-size: calc(var(--size, 3rem) * 0.5)"
			></wa-icon>
		{/if}
	</wa-avatar>
</span>
