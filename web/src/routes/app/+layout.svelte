<script lang="ts">
	import { page } from '$app/state';
	import {
		DashboardSquare01Icon,
		Film01Icon,
		PlayListIcon,
		LiveStreaming01Icon,
		UserGroupIcon,
		Analytics01Icon,
		Settings01Icon
	} from '@hugeicons/core-free-icons';
	import { Icon, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import Auth from '$lib/components/Auth.svelte';
	import UserMenu from '$lib/components/UserMenu.svelte';

	let { children } = $props();

	const nav = [
		{ href: '/app', label: 'Dashboard', icon: DashboardSquare01Icon },
		{ href: '/app/assets', label: 'Assets', icon: Film01Icon },
		{ href: '/app/jobs', label: 'Jobs', icon: PlayListIcon },
		{ href: '/app/live', label: 'Live', icon: LiveStreaming01Icon },
		{ href: '/app/metrics', label: 'Metrics', icon: Analytics01Icon },
		{ href: '/app/team', label: 'Team', icon: UserGroupIcon },
		{ href: '/app/settings', label: 'Settings', icon: Settings01Icon }
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
	</aside>

	<!-- Main -->
	<div class="flex min-w-0 flex-1 flex-col">
		<header class="flex h-16 items-center justify-between border-b border-border px-6">
			<div class="md:hidden">
				<Logo size={24} />
			</div>
			<div class="ml-auto">
				<UserMenu />
			</div>
		</header>
		<main class="flex-1 px-6 py-8">
			{@render children()}
		</main>
	</div>
</div>
{/if}
