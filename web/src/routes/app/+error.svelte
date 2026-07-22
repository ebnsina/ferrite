<script lang="ts">
	import { page } from '$app/state';
	import { Button, Icon } from '$lib/ui';
	import { FileNotFoundIcon, ServerStack01Icon, Alert02Icon } from '@hugeicons/core-free-icons';

	const status = $derived(page.status);
	const isNotFound = $derived(status === 404);
	const isServer = $derived(status >= 500);

	const title = $derived(
		isNotFound ? 'Page not found' : isServer ? 'Something broke' : 'Something went wrong'
	);

	const detail = $derived(
		isNotFound
			? "The page you're looking for doesn't exist or has moved."
			: (page.error?.message ?? 'An unexpected error occurred.')
	);
</script>

<div class="flex min-h-[70vh] flex-col items-center justify-center px-6 text-center">
	<div class="mb-6 rounded-full border border-border bg-surface p-5 text-accent">
		{#if isNotFound}
			<Icon icon={FileNotFoundIcon} size={40} />
		{:else if isServer}
			<Icon icon={ServerStack01Icon} size={40} />
		{:else}
			<Icon icon={Alert02Icon} size={40} />
		{/if}
	</div>

	<p class="mono mb-2 text-sm text-muted">Error {status}</p>
	<h1 class="mb-2 text-2xl font-semibold">{title}</h1>
	<p class="mb-6 max-w-md text-muted">{detail}</p>

	{#if page.error?.errorId}
		<p class="mono mb-6 text-xs text-muted">Reference: {page.error.errorId}</p>
	{/if}

	<div class="flex gap-3">
		<Button variant="secondary" onclick={() => history.back()}>Go back</Button>
		<a href="/app"><Button>Dashboard</Button></a>
	</div>
</div>
