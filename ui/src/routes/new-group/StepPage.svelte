<script lang="ts">
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Button,
		Link,
		useTheme,
	} from 'konsta/svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		onBack?: () => void;
		backTestId?: string;
		actionLabel: string;
		onAction: () => void;
		actionTestId?: string;
		subnavbar?: Snippet;
		children: Snippet;
	}

	let {
		title,
		onBack,
		backTestId,
		actionLabel,
		onAction,
		actionTestId,
		subnavbar: belowNavbar,
		children,
	}: Props = $props();

	const theme = $derived(useTheme());
	const isIosTheme = $derived(theme === 'ios');
</script>

<Page>
	<Navbar
		{title}
		titleClass="opacity1"
		transparent={true}
		subnavbarClass={belowNavbar ? '!h-auto' : ''}
	>
		{#snippet left()}
			<NavbarBackLink
				onClick={onBack ?? (() => window.history.back())}
				data-testid={backTestId}
			/>
		{/snippet}

		{#snippet right()}
			{#if isIosTheme}
				<Link onClick={onAction} data-testid={actionTestId}>
					{actionLabel}
				</Link>
			{/if}
		{/snippet}

		{#snippet subnavbar()}
			{#if belowNavbar}
				<div class="w-full mb-4 {isIosTheme ? 'mt-4' : ''}">
					{@render belowNavbar()}
				</div>
			{/if}
		{/snippet}
	</Navbar>

	{@render children()}

	{#if !isIosTheme}
		<Button
			onClick={onAction}
			data-testid={actionTestId}
			class="fixed-action-btn"
			rounded
		>
			{actionLabel}
		</Button>
	{/if}
</Page>
