<script lang="ts">
	import { page } from '$app/state';
	import {
		DashboardSquare01Icon,
		Film01Icon,
		PlayListIcon,
		LiveStreaming01Icon,
		UserGroupIcon,
		Analytics01Icon,
		Settings01Icon,
		Search01Icon,
		Building01Icon,
		PulseIcon,
		CreditCardIcon,
		KeyframeIcon,
		Menu01Icon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import { afterNavigate } from '$app/navigation';
	import { fly, fade } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { Icon, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { dur } from '$lib/motion';
	import Auth from '$lib/components/Auth.svelte';
	import UserMenu from '$lib/components/UserMenu.svelte';

	let { children } = $props();

	let mobileOpen = $state(false);
	afterNavigate(() => (mobileOpen = false));

	// Grouped navigation. The first group has no heading (primary landing);
	// the rest are labelled sections for quick scanning.
	const groups = $derived([
		{
			label: null,
			items: [{ href: '/app', label: 'Overview', icon: DashboardSquare01Icon }]
		},
		{
			label: 'Content',
			items: [
				{ href: '/app/assets', label: 'Videos', icon: Film01Icon },
				{ href: '/app/jobs', label: 'Jobs', icon: PlayListIcon },
				{ href: '/app/search', label: 'Search', icon: Search01Icon }
			]
		},
		{
			label: 'Streaming',
			items: [{ href: '/app/live', label: 'Live', icon: LiveStreaming01Icon }]
		},
		{
			label: 'Insights',
			items: [
				{ href: '/app/analytics', label: 'Analytics', icon: Analytics01Icon },
				{ href: '/app/activity', label: 'Activity', icon: PulseIcon }
			]
		},
		{
			label: 'Workspace',
			items: [
				{ href: '/app/team', label: 'Team', icon: UserGroupIcon },
				{ href: '/app/keys', label: 'API keys', icon: KeyframeIcon },
				{ href: '/app/usage', label: 'Usage', icon: CreditCardIcon },
				{ href: '/app/settings', label: 'Settings', icon: Settings01Icon },
				...(session.user?.superadmin
					? [{ href: '/admin', label: 'Admin', icon: Building01Icon }]
					: [])
			]
		}
	]);

	function isActive(href: string) {
		return href === '/app' ? page.url.pathname === '/app' : page.url.pathname.startsWith(href);
	}
</script>

{#snippet navList()}
	<nav class="flex flex-1 flex-col gap-5 overflow-y-auto p-3">
		{#each groups as group, gi (gi)}
			<div class="flex flex-col gap-0.5">
				{#if group.label}
					<p class="px-3 pb-1.5 text-[10px] font-semibold tracking-wider text-muted/70 uppercase">
						{group.label}
					</p>
				{/if}
				{#each group.items as item (item.href)}
					<a
						href={item.href}
						class={`group relative flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
							isActive(item.href)
								? 'bg-accent-soft font-medium text-accent'
								: 'text-muted hover:bg-surface-2 hover:text-fg'
						}`}
					>
						{#if isActive(item.href)}
							<span class="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-accent"></span>
						{/if}
						<Icon icon={item.icon} size={18} />
						{item.label}
					</a>
				{/each}
			</div>
		{/each}
	</nav>
{/snippet}

{#if !session.isAuthed}
	<Auth />
{:else}
	<div class="flex h-screen overflow-hidden">
		<!-- Sidebar (fixed height; only its own nav scrolls if it overflows) -->
		<aside class="hidden w-60 shrink-0 border-r border-border bg-surface md:flex md:flex-col">
			<div class="flex h-16 items-center border-b border-border px-5">
				<a href="/app"><Logo size={26} /></a>
			</div>
			{@render navList()}
		</aside>

		<!-- Mobile nav drawer -->
		{#if mobileOpen}
			<button
				class="fixed inset-0 z-40 bg-black/50 backdrop-blur-sm md:hidden"
				aria-label="Close menu"
				onclick={() => (mobileOpen = false)}
				transition:fade={{ duration: dur(180) }}
			></button>
			<aside
				class="fixed inset-y-0 left-0 z-50 flex w-64 flex-col border-r border-border bg-surface md:hidden"
				transition:fly={{ x: -280, duration: dur(260), easing: cubicOut }}
			>
				<div class="flex h-16 items-center justify-between border-b border-border px-5">
					<a href="/app"><Logo size={24} /></a>
					<button
						onclick={() => (mobileOpen = false)}
						aria-label="Close menu"
						class="rounded-lg p-1.5 text-muted hover:bg-surface-2 hover:text-fg"
					>
						<Icon icon={Cancel01Icon} size={18} />
					</button>
				</div>
				{@render navList()}
			</aside>
		{/if}

		<!-- Main (header pinned; content scrolls independently) -->
		<div class="flex min-w-0 flex-1 flex-col">
			<header class="flex h-16 shrink-0 items-center justify-between border-b border-border px-6">
				<div class="flex items-center gap-2 md:hidden">
					<button
						onclick={() => (mobileOpen = true)}
						aria-label="Open menu"
						class="rounded-lg p-1.5 text-muted hover:bg-surface-2 hover:text-fg"
					>
						<Icon icon={Menu01Icon} size={20} />
					</button>
					<Logo size={24} />
				</div>
				<div class="ml-auto">
					<UserMenu />
				</div>
			</header>
			<main class="flex-1 overflow-y-auto px-6 py-8">
				{#key page.url.pathname}
					<div in:fly={{ y: 10, duration: dur(220), easing: cubicOut }}>
						{@render children()}
					</div>
				{/key}
			</main>
		</div>
	</div>
{/if}
