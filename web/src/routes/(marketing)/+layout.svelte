<script lang="ts">
	import { Icon } from '$lib/ui';
	import { ApertureIcon } from '@hugeicons/core-free-icons';

	let { children } = $props();

	const links = [
		{ href: '/#features', label: 'Features' },
		{ href: '/#how', label: 'How it works' },
		{ href: '/pricing', label: 'Pricing' }
	];
</script>

<div class="flex min-h-screen flex-col">
	<header class="sticky top-0 z-20 border-b border-border bg-bg/80 backdrop-blur">
		<div class="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
			<a href="/" class="flex items-center gap-2">
				<span class="text-accent"><Icon icon={ApertureIcon} size={22} /></span>
				<span class="text-lg font-semibold tracking-tight">Ferrite</span>
			</a>
			<nav class="hidden items-center gap-8 md:flex">
				{#each links as l (l.href)}
					<a href={l.href} class="text-sm text-muted transition-colors hover:text-fg">{l.label}</a>
				{/each}
			</nav>
			<div class="flex items-center gap-2">
				<a
					href="/app"
					class="hidden rounded-lg px-3 py-2 text-sm text-muted transition-colors hover:text-fg sm:block"
					>Sign in</a
				>
				<a
					href="/app"
					class="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
					>Get started</a
				>
			</div>
		</div>
	</header>

	<main class="flex-1">
		{@render children()}
	</main>

	<footer class="border-t border-border">
		<div
			class="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 px-6 py-8 sm:flex-row"
		>
			<div class="flex items-center gap-2">
				<span class="text-accent"><Icon icon={ApertureIcon} size={18} /></span>
				<span class="text-sm font-medium">Ferrite</span>
				<span class="text-sm text-muted">— multi-tenant video transcoding.</span>
			</div>
			<div class="flex items-center gap-6 text-sm text-muted">
				<a href="/pricing" class="hover:text-fg">Pricing</a>
				<a href="/app" class="hover:text-fg">Dashboard</a>
				<span>© {new Date().getFullYear()}</span>
			</div>
		</div>
	</footer>
</div>
