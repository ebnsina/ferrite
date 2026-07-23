<script lang="ts">
	import { goto } from '$app/navigation';
	import { Icon, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import UserMenu from '$lib/components/UserMenu.svelte';
	import { ArrowLeft01Icon } from '@hugeicons/core-free-icons';

	let { children } = $props();

	// Gate: only platform superadmins; everyone else goes back to the app.
	$effect(() => {
		if (!session.isAuthed || !session.user?.superadmin) goto('/app');
	});
</script>

{#if session.isAuthed && session.user?.superadmin}
	<div class="flex min-h-screen flex-col">
		<header class="flex h-16 items-center justify-between border-b border-border px-6">
			<div class="flex items-center gap-3">
				<a href="/admin"><Logo size={24} /></a>
				<span class="mono rounded-full bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent"
					>ADMIN</span
				>
			</div>
			<div class="flex items-center gap-3">
				<a
					href="/app"
					class="inline-flex items-center gap-1.5 text-sm text-muted transition-colors hover:text-fg"
				>
					<Icon icon={ArrowLeft01Icon} size={15} /> Back to app
				</a>
				<UserMenu />
			</div>
		</header>
		<main class="flex-1 px-6 py-8">
			{@render children()}
		</main>
	</div>
{/if}
