<script lang="ts">
	import {
		Actions,
		ActionsGroup,
		Dialog,
		DialogButton,
		Preloader,
	} from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from './ActionButton.svelte';
	import ActionsTitle from './ActionsTitle.svelte';
	import { isIos } from '$lib/utils/environment';
	import Modal from '$lib/components/Modal.svelte';

	export interface ActionDialogAction {
		text: string;
		onClick: () => void | Promise<void>;
		destructive?: boolean;
		strong?: boolean;
		testid?: string;
	}

	interface Props {
		title: string;
		description?: string;
		/** Primary actions, most prominent first. Cancel is appended
		 * automatically. */
		actions: ActionDialogAction[];
		cancelText?: string;
		cancelTestId?: string;
		onCancel?: () => void;
		/** Called on every close, no matter which path triggered it. */
		onClosed?: () => void;
		testid?: string;
	}

	let {
		title,
		description,
		actions,
		cancelText = m.cancel(),
		cancelTestId,
		onCancel,
		onClosed,
		testid,
	}: Props = $props();

	let opened = $state(false);
	let loading = $state(false);
	let running = $state<ActionDialogAction | null>(null);

	export function show() {
		opened = true;
	}

	export function close() {
		opened = false;
		onClosed?.();
	}

	function cancel() {
		close();
		onCancel?.();
	}

	async function run(action: ActionDialogAction) {
		const result = action.onClick();
		if (!(result instanceof Promise)) return;
		loading = true;
		running = action;
		try {
			await result;
		} finally {
			loading = false;
			running = null;
		}
	}
</script>

{#snippet actionPreloader(action: ActionDialogAction)}
	{#if running === action}
		<Preloader class="ms-2 h-4 w-4" />
	{/if}
{/snippet}

{#snippet cancelButton()}
	<DialogButton onClick={cancel} disabled={loading} data-testid={cancelTestId}>
		{cancelText}
	</DialogButton>
{/snippet}

{#snippet dialogButton(action: ActionDialogAction)}
	<DialogButton
		class={action.destructive ? '!text-red-500' : ''}
		strong={action.strong}
		onClick={() => run(action)}
		disabled={loading}
		data-testid={action.testid}
	>
		{action.text}
		{@render actionPreloader(action)}
	</DialogButton>
{/snippet}

<Modal bind:opened>
	{#snippet children(modal)}
		{#if isIos}
			<Actions
				opened={modal.opened}
				onBackdropClick={cancel}
				data-testid={testid}
			>
				<ActionsGroup
					class="flex flex-col gap-2 !bg-white p-2.5 dark:!bg-neutral-900"
				>
					<ActionsTitle {title} subtitle={description} />
					{#each actions as action (action.text)}
						<ActionButton
							destructive={action.destructive}
							onClick={() => run(action)}
							disabled={loading}
							data-testid={action.testid}
						>
							{action.text}
							{@render actionPreloader(action)}
						</ActionButton>
					{/each}
					<ActionButton
						onClick={cancel}
						disabled={loading}
						data-testid={cancelTestId}
					>
						{cancelText}
					</ActionButton>
				</ActionsGroup>
			</Actions>
		{:else}
			<Dialog
				opened={modal.opened}
				onBackdropClick={cancel}
				{title}
				data-testid={testid}
			>
				{#if description}
					<span>{description}</span>
				{/if}
				{#snippet buttons()}
					{#if actions.length > 1}
						<div class="flex w-full flex-col items-end gap-2">
							{#each actions as action (action.text)}
								{@render dialogButton(action)}
							{/each}
							{@render cancelButton()}
						</div>
					{:else}
						{@render cancelButton()}
						{#each actions as action (action.text)}
							{@render dialogButton(action)}
						{/each}
					{/if}
				{/snippet}
			</Dialog>
		{/if}
	{/snippet}
</Modal>
