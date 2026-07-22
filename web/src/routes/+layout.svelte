<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { page } from '$app/state';
	import { LayoutDashboard, Film, ListVideo, Moon, Sun, Zap, LogOut } from '@lucide/svelte';
	import { session } from '$lib/api/session.svelte';
	import Connect from '$lib/components/Connect.svelte';

	let { children } = $props();

	let theme = $state<'dark' | 'light'>('dark');

	function toggleTheme() {
		theme = theme === 'dark' ? 'light' : 'dark';
		document.documentElement.setAttribute('data-theme', theme);
	}

	const nav = [
		{ href: '/', label: 'Dashboard', icon: LayoutDashboard },
		{ href: '/assets', label: 'Assets', icon: Film },
		{ href: '/jobs', label: 'Jobs', icon: ListVideo }
	];

	function isActive(href: string) {
		return href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
	}
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

{#if !session.isAuthed}
	<Connect />
{:else}

<div class="flex min-h-screen">
	<!-- Sidebar -->
	<aside class="hidden w-60 shrink-0 border-r border-border bg-surface md:flex md:flex-col">
		<div class="flex h-16 items-center gap-2 border-b border-border px-5">
			<span class="text-accent"><Zap size={20} /></span>
			<span class="text-lg font-semibold tracking-tight">Ferrite</span>
		</div>
		<nav class="flex flex-1 flex-col gap-1 p-3">
			{#each nav as item (item.href)}
				<a
					href={item.href}
					class={`flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
						isActive(item.href)
							? 'bg-accent-soft text-accent'
							: 'text-muted hover:bg-surface-2 hover:text-fg'
					}`}
				>
					<item.icon size={18} />
					{item.label}
				</a>
			{/each}
		</nav>
	</aside>

	<!-- Main -->
	<div class="flex min-w-0 flex-1 flex-col">
		<header class="flex h-16 items-center justify-between border-b border-border px-6">
			<div class="flex items-center gap-2 md:hidden">
				<span class="text-accent"><Zap size={18} /></span>
				<span class="font-semibold">Ferrite</span>
			</div>
			<div class="ml-auto flex items-center gap-1">
				<button
					onclick={toggleTheme}
					aria-label="Toggle theme"
					class="rounded-lg p-2 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
				>
					{#if theme === 'dark'}<Sun size={18} />{:else}<Moon size={18} />{/if}
				</button>
				<button
					onclick={() => session.clear()}
					aria-label="Sign out"
					class="rounded-lg p-2 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
				>
					<LogOut size={18} />
				</button>
			</div>
		</header>
		<main class="flex-1 px-6 py-8">
			{@render children()}
		</main>
	</div>
</div>
{/if}
