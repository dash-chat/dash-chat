<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiAccountGroup, mdiAccountQuestion } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TextAvatarData } from './text-avatar-data-url';
	import {
		abbreviateName,
		defaultAvatarColor,
		measureInitialsLayout,
		TEXT_AVATAR_TEXT_COLOR,
	} from './avatar-helpers';
	import type { Snippet } from 'svelte';

	let {
		waitingForProfile,
		image,
		name,
		colorSeed,
		group = false,
		alt,
		size,
		style,
		id,
		children,
	}: {
		waitingForProfile?: boolean | undefined;
		image?: string | undefined;
		name?: string | undefined;
		colorSeed?: string | undefined;
		group?: boolean | undefined;
		alt?: string | undefined;
		size?: number | string | undefined;
		style?: string | undefined;
		id?: string | undefined;
		children?: Snippet | undefined;
	} = $props();

	const avatarImage = $derived(
		image?.startsWith('data:image') ? image : undefined,
	);

	// A group with no photo renders a group glyph on a stable hashed color, like
	// Signal — never the name's initials.
	const showGroupGlyph = $derived(group && !avatarImage);

	// A profile with no avatar gets a virtual text avatar, like Signal: its
	// initials on a stable color from the text-avatar palette. Never
	// serialized, so initials the stored format rejects still render.
	const textAvatarData = $derived.by(() => {
		if (avatarImage || group) {
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
		const tint = showGroupGlyph
			? defaultAvatarColor(colorSeed || name || '')
			: textAvatarData?.sanitizedHexColor();
		if (!tint) {
			return baseStyle;
		}

		const tintStyle = `background-color: ${tint}; color: ${TEXT_AVATAR_TEXT_COLOR};`;
		return baseStyle ? `${baseStyle}; ${tintStyle}` : tintStyle;
	});

	// Centre the initials on their real ink box (and shrink wide labels) the way
	// Signal's native apps do, since CSS metrics alone can't centre lower-case
	// or CJK glyphs. Re-measured against the rendered diameter and font.
	let avatarEl = $state<HTMLElement>();
	let initialsVars = $state('');
	$effect(() => {
		const text = textAvatarData?.text;
		const el = avatarEl;
		void sizeValue;
		if (!text || !el) {
			initialsVars = '';
			return;
		}
		const diameter = el.getBoundingClientRect().width;
		const { fontSizePx, translateYPx } = measureInitialsLayout(
			text,
			diameter,
			getComputedStyle(el).fontFamily,
		);
		initialsVars = `--initials-size: ${fontSizePx}px; --initials-dy: ${translateYPx}px;`;
	});

	const fullStyle = $derived(
		[avatarStyle, initialsVars].filter(Boolean).join(' '),
	);
</script>

<span class="inline-block">
	<wa-avatar
		bind:this={avatarEl}
		{id}
		image={avatarImage}
		initials={textAvatarData?.text}
		style={fullStyle}
		{alt}
		shape="circle"
	>
		{#if showGroupGlyph}
			<wa-icon
				slot="icon"
				src={wrapPathInSvg(mdiAccountGroup)}
				style="font-size: calc(var(--size, 3rem) * 0.6)"
			></wa-icon>
		{:else if waitingForProfile}
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
	/* Signal's text-avatar proportions: initials at 0.45 of the circle in a
	   medium weight (Signal-iOS/Android render Inter Medium for cross-platform
	   parity; webawesome's default is 0.4 and inherited weight). --initials-size
	   is overridden per-label by measureInitialsLayout to shrink wide labels. */
	wa-avatar {
		font-size: calc(var(--size, 3rem) * 0.45);
		font-weight: 500;
	}

	/* webawesome force-uppercases initials; Signal preserves the name's case.
	   text-box trims the box to cap-top/baseline so flexbox parks the baseline
	   half a cap-height below centre; --initials-dy then nudges the measured ink
	   box onto the circle's true centre (Signal's content-aware centering). */
	wa-avatar::part(initials) {
		text-transform: none;
		text-box: trim-both cap alphabetic;
		font-size: var(--initials-size, calc(var(--size, 3rem) * 0.45));
		translate: 0 var(--initials-dy, 0px);
	}
</style>
