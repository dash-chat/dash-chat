<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiAccountQuestion } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TextAvatarData } from './text-avatar-data-url';
	import {
		abbreviateName,
		defaultAvatarColor,
		TEXT_AVATAR_TEXT_COLOR,
	} from './avatar-helpers';
	import type { Snippet } from 'svelte';

	let {
		waitingForProfile,
		image,
		name,
		colorSeed,
		alt,
		style,
		id,
		children,
	}: {
		waitingForProfile?: boolean | undefined;
		image?: string | undefined;
		name?: string | undefined;
		colorSeed?: string | undefined;
		alt?: string | undefined;
		style?: string | undefined;
		id?: string | undefined;
		children?: Snippet | undefined;
	} = $props();

	const avatarImage = $derived(
		image?.startsWith('data:image') ? image : undefined,
	);
	// A profile with no avatar gets a virtual text avatar, like Signal: its
	// initials on a stable color from the text-avatar palette. Never
	// serialized, so initials the stored format rejects still render.
	const textAvatarData = $derived.by(() => {
		if (avatarImage) {
			return undefined;
		}
		if (name?.trim()) {
			return (
				TextAvatarData.deserialize(image) ??
				new TextAvatarData(
					defaultAvatarColor(colorSeed || name),
					abbreviateName(name),
				)
			);
		}
		return TextAvatarData.deserialize(image);
	});
	const avatarStyle = $derived.by(() => {
		if (!textAvatarData) {
			return style;
		}

		const textAvatarStyle = `background-color: ${textAvatarData.sanitizedHexColor()}; color: ${TEXT_AVATAR_TEXT_COLOR};`;
		return style ? `${style}; ${textAvatarStyle}` : textAvatarStyle;
	});
</script>

<span class="inline-block">
	<wa-avatar
		{id}
		image={avatarImage}
		initials={textAvatarData?.text}
		style={avatarStyle}
		{alt}
		shape="circle"
	>
		{#if waitingForProfile}
			<wa-icon
				slot="icon"
				src={wrapPathInSvg(mdiAccountQuestion)}
				style="font-size: calc(var(--size, 3rem) * 0.5)"
			></wa-icon>
		{/if}
		{@render children?.()}
	</wa-avatar>
</span>

<style>
	/* Signal-like initials proportions: larger and semibold relative to the
	   circle (webawesome's default is 0.4 and inherited weight). */
	wa-avatar {
		font-size: calc(var(--size, 3rem) * 0.45);
		font-weight: 600;
	}
</style>
