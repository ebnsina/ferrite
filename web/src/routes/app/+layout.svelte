<script lang="ts">
	import { page } from '$app/state';
	import {
		DashboardSquare01Icon,
		Film01Icon,
		PlayListIcon,
		LiveStreaming01Icon,
		UserGroupIcon,
		Analytics01Icon,
		Moon02Icon,
		Sun03Icon,
		Logout01Icon
	} from '@hugeicons/core-free-icons';
	import { browser } from '$app/environment';
	import { Icon, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import Auth from '$lib/components/Auth.svelte';

	let { children } = $props();

	// Initial theme is set by the inline script in app.html (before paint); mirror it here.
	let theme = $state<'dark' | 'light'>(
		browser && document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark'
	);

	function toggleTheme() {
		theme = theme === 'dark' ? 'light' : 'dark';
		document.documentElement.setAttribute('data-theme', theme);
		localStorage.setItem('ferrite.theme', theme);
	}

	const nav = [
		{ href: '/app', label: 'Dashboard', icon: DashboardSquare01Icon },
		{ href: '/app/assets', label: 'Assets', icon: Film01Icon },
		{ href: '/app/jobs', label: 'Jobs', icon: PlayListIcon },
		{ href: '/app/live', label: 'Live', icon: LiveStreaming01Icon },
		{ href: '/app/metrics', label: 'Metrics', icon: Analytics01Icon },
		{ href: '/app/team', label: 'Team', icon: UserGroupIcon }
	];

	function isActive(href: string) {
		return href === '/app' ? page.url.pathname === '/app' : page.url.pathname.startsWith(href);
	}
</script>

{#if !session.isAuthed}
	<Auth />
{:else}

<div class="flex min-h-screen">
	<!-- Sidebar -->
	<aside class="hidden w-60 shrink-0 border-r border-border bg-surface md:flex md:flex-col">
		<div class="flex h-16 items-center border-b border-border px-5">
			<a href="/app"><Logo size={26} /></a>
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
					<Icon icon={item.icon} size={18} />
					{item.label}
				</a>
			{/each}
		</nav>
		{#if session.tenant}
			<div class="border-t border-border p-4">
				<p class="truncate text-sm font-medium">{session.tenant.name}</p>
				<p class="truncate text-xs text-muted">{session.user?.email}</p>
			</div>
		{/if}
	</aside>

	<!-- Main -->
	<div class="flex min-w-0 flex-1 flex-col">
		<header class="flex h-16 items-center justify-between border-b border-border px-6">
			<div class="md:hidden">
				<Logo size={24} />
			</div>
			<div class="ml-auto flex items-center gap-1">
				<button
					onclick={toggleTheme}
					aria-label="Toggle theme"
					class="rounded-lg p-2 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
				>
					{#if theme === 'dark'}<Icon icon={Sun03Icon} size={18} />{:else}<Icon
							icon={Moon02Icon}
							size={18}
						/>{/if}
				</button>
				<button
					onclick={() => session.clear()}
					aria-label="Sign out"
					class="rounded-lg p-2 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
				>
					<Icon icon={Logout01Icon} size={18} />
				</button>
			</div>
		</header>
		<main class="flex-1 px-6 py-8">
			{@render children()}
		</main>
	</div>
</div>
{/if}
