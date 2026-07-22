<script lang="ts">
	import { Card, Button, StatusPill, ProgressBar } from '$lib/ui';
	import { Upload, Film, ListVideo, HardDrive } from '@lucide/svelte';
	import type { JobState } from '$lib/api/types';

	// Placeholder data until the API endpoints land (Phase 1).
	const stats = [
		{ label: 'Assets', value: '0', icon: Film },
		{ label: 'Active jobs', value: '0', icon: ListVideo },
		{ label: 'Storage', value: '0 GB', icon: HardDrive }
	];

	const recent: { id: string; name: string; state: JobState; progress: number }[] = [];
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Dashboard</h1>
			<p class="mt-1 text-sm text-muted">Transcode overview for your workspace.</p>
		</div>
		<Button><Upload size={16} /> Upload video</Button>
	</div>

	<div class="mb-8 grid gap-4 sm:grid-cols-3">
		{#each stats as stat (stat.label)}
			<Card>
				<div class="flex items-center justify-between">
					<div>
						<p class="text-sm text-muted">{stat.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{stat.value}</p>
					</div>
					<span class="text-muted"><stat.icon size={22} /></span>
				</div>
			</Card>
		{/each}
	</div>

	<Card>
		<h2 class="mb-4 text-sm font-medium text-muted">Recent jobs</h2>
		{#if recent.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><ListVideo size={32} /></span>
				<p class="font-medium">No jobs yet</p>
				<p class="mt-1 text-sm text-muted">Upload a video to kick off your first transcode.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each recent as job (job.id)}
					<div class="flex items-center gap-4 py-3">
						<span class="min-w-0 flex-1 truncate text-sm">{job.name}</span>
						<div class="w-40"><ProgressBar value={job.progress} /></div>
						<StatusPill state={job.state} />
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>
