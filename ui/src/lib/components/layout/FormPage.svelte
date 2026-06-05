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
		navbarTestId?: string;
		backTestId?: string;
		actionLabel: string;
		onAction: () => void;
		actionDisabled?: boolean;
		actionTestId?: string;
		subnavbar?: Snippet;
		constrainedWidth?: boolean;
		children: Snippet;
	}

	let {
		title,
		onBack,
		backTestId,
		actionLabel,
		onAction,
		actionDisabled = false,
		actionTestId,
		navbarTestId,
		subnavbar: belowNavbar,
		constrainedWidth,
		children,
	}: Props = $props();

	function handleAction() {
		if (actionDisabled) return;
		onAction();
	}

	const theme = $derived(useTheme());
	const isIosTheme = $derived(theme === 'ios');
</script>

{#snippet subnavbarSnippet()}
	{#if belowNavbar}
		<div class="w-full mb-4 {isIosTheme ? 'mt-4' : ''}">
			{#if constrainedWidth}
				<div class="center-in-desktop">
					{@render belowNavbar()}
				</div>
			{:else}
				{@render belowNavbar()}
			{/if}
		</div>
	{/if}
{/snippet}

<Page>
	<Navbar
		{title}
		titleClass="opacity1"
		transparent={true}
		subnavbarClass={belowNavbar ? '!h-auto' : ''}
		rightClass={actionDisabled ? 'ios-right-disabled' : ''}
		data-testid={navbarTestId}
		subnavbar={belowNavbar ? subnavbarSnippet : undefined}
	>
		{#snippet left()}
			<NavbarBackLink
				onClick={onBack ?? (() => window.history.back())}
				data-testid={backTestId}
			/>
		{/snippet}

		{#snippet right()}
			{#if isIosTheme}
				<Link
					onClick={onAction}
					data-testid={actionTestId}
					class={actionDisabled ? 'ios-right-disabled' : ''}
				>
					{actionLabel}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	{#if constrainedWidth}
		<div class="column">
			<div class="center-in-desktop">
				{@render children()}
			</div>
		</div>
	{:else}
		{@render children()}
	{/if}

	{#if !isIosTheme}
		<Button
			onClick={onAction}
			data-testid={actionTestId}
			class="fixed-action-btn"
			rounded
			disabled={actionDisabled}
		>
			{actionLabel}
		</Button>
	{/if}
</Page>
