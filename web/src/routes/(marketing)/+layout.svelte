<script lang="ts">
	import { Logo } from '$lib/ui';

	let { children } = $props();

	// Smooth-scroll to a section by id without pushing a #hash onto the URL.
	function scrollTo(id: string) {
		document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	const nav = [
		{ id: 'features', label: 'Features' },
		{ id: 'benefits', label: 'Why Ferrite Stream' },
		{ id: 'pricing', label: 'Pricing' }
	];

	const footerCols = [
		{
			title: 'Product',
			links: [
				{ scroll: 'features', label: 'Features' },
				{ scroll: 'benefits', label: 'Why Ferrite Stream' },
				{ scroll: 'how', label: 'How it works' },
				{ scroll: 'pricing', label: 'Pricing' }
			]
		},
		{
			title: 'Platform',
			links: [
				{ scroll: 'features', label: 'HLS & DASH' },
				{ scroll: 'features', label: 'Live streaming' },
				{ scroll: 'features', label: 'Signed playback' },
				{ scroll: 'features', label: 'GPU encoding' }
			]
		},
		{
			title: 'Get started',
			links: [
				{ href: '/app', label: 'Sign in' },
				{ href: '/app', label: 'Create workspace' },
				{ scroll: 'pricing', label: 'View pricing' }
			]
		}
	];
</script>

<div class="flex min-h-screen flex-col">
	<!-- Transparent header blends into the hero; no divider. -->
	<header class="absolute inset-x-0 top-0 z-20">
		<div class="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
			<a href="/" aria-label="Ferrite Stream home"><Logo /></a>
			<nav class="hidden items-center gap-8 md:flex">
				{#each nav as l (l.label)}
					<button
						onclick={() => scrollTo(l.id)}
						class="text-sm text-muted transition-colors hover:text-fg">{l.label}</button
					>
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

	<!-- Full-bleed footer band. -->
	<footer class="w-full border-t border-border bg-surface/60">
		<div class="mx-auto max-w-6xl px-6 py-16">
			<div class="grid gap-10 md:grid-cols-[1.4fr_1fr_1fr_1fr]">
				<div>
					<Logo />
					<p class="mt-4 max-w-xs text-sm text-muted">
						Self-hosted, adaptive video transcoding for teams shipping VOD and live.
					</p>
				</div>
				{#each footerCols as col (col.title)}
					<div>
						<h3 class="mb-3 text-xs font-semibold tracking-wide text-muted uppercase">{col.title}</h3>
						<ul class="flex flex-col gap-2.5">
							{#each col.links as l (l.label)}
								<li>
									{#if l.href}
										<a href={l.href} class="text-sm text-muted transition-colors hover:text-fg"
											>{l.label}</a
										>
									{:else}
										<button
											onclick={() => scrollTo(l.scroll!)}
											class="text-sm text-muted transition-colors hover:text-fg">{l.label}</button
										>
									{/if}
								</li>
							{/each}
						</ul>
					</div>
				{/each}
			</div>
			<div
				class="mt-12 flex flex-col items-center justify-between gap-3 border-t border-border pt-6 text-sm text-muted sm:flex-row"
			>
				<span>© {new Date().getFullYear()} Ferrite. All rights reserved.</span>
				<span class="mono text-xs">HLS · DASH · CMAF · SRT · NVENC-ready</span>
			</div>
		</div>
	</footer>
</div>
