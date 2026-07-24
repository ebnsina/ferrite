<script lang="ts">
	import { page } from '$app/state';
	import { Button, Icon, Logo } from '$lib/ui';
	import { FileDashed, Stack, Warning } from 'phosphor-svelte';

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

<div class="flex min-h-screen flex-col">
	<header class="border-b border-border">
		<div class="mx-auto flex h-16 max-w-6xl items-center px-6">
			<a href="/" aria-label="Ferrite Stream home"><Logo /></a>
		</div>
	</header>

	<div class="flex flex-1 flex-col items-center justify-center px-6 text-center">
		<div class="mb-6 rounded-full border border-border bg-surface p-5 text-accent">
			{#if isNotFound}
				<Icon icon={FileDashed} size={40} />
			{:else if isServer}
				<Icon icon={Stack} size={40} />
			{:else}
				<Icon icon={Warning} size={40} />
			{/if}
		</div>

		<p class="mono mb-2 text-sm text-muted">Error {status}</p>
		<h1 class="mb-2 text-2xl font-semibold">{title}</h1>
		<p class="mb-6 max-w-md text-muted">{detail}</p>

		<div class="flex gap-3">
			<Button variant="secondary" onclick={() => history.back()}>Go back</Button>
			<a href="/"><Button>Home</Button></a>
		</div>
	</div>
</div>
