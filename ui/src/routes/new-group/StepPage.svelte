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
		actionTestId?: string;
		actionDisabled?: boolean;
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
		actionDisabled = false,
		navbarTestId,
		subnavbar: belowNavbar,
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
			{@render belowNavbar()}
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
				<Link onClick={handleAction} data-testid={actionTestId}>
					{actionLabel}
				</Link>
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
			disabled={actionDisabled}
		>
			{actionLabel}
		</Button>
	{/if}
</Page>
