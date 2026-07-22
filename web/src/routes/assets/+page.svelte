<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { Card, Button, Icon } from '$lib/ui';
	import { listAssets, createAsset, uploadToPresigned, completeAsset, createJob } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Asset } from '$lib/api/types';
	import { bytes, timeAgo } from '$lib/format';
	import { Upload01Icon, Film01Icon, Loading03Icon } from '@hugeicons/core-free-icons';

	let assets = $state<Asset[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let uploading = $state(false);
	let fileInput: HTMLInputElement;

	async function load() {
		try {
			assets = await listAssets();
			error = null;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load assets.';
		} finally {
			loading = false;
		}
	}
	onMount(load);

	async function onFile(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		uploading = true;
		error = null;
		try {
			const { asset, upload_url } = await createAsset(file.name);
			await uploadToPresigned(upload_url, file);
			await completeAsset(asset.id, file.size);
			await load();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Upload failed.';
		} finally {
			uploading = false;
			fileInput.value = '';
		}
	}

	async function transcode(assetId: string) {
		try {
			await createJob(assetId);
			goto('/jobs');
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not start transcode.';
		}
	}
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Assets</h1>
			<p class="mt-1 text-sm text-muted">Upload source videos to transcode.</p>
		</div>
		<input bind:this={fileInput} type="file" accept="video/*" class="hidden" onchange={onFile} />
		<Button disabled={uploading} onclick={() => fileInput.click()}>
			{#if uploading}<Icon icon={Loading03Icon} size={16} class="animate-spin" /> Uploading…{:else}<Icon
					icon={Upload01Icon}
					size={16}
				/> Upload video{/if}
		</Button>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<Card>
		{#if loading}
			<p class="py-8 text-center text-sm text-muted">Loading…</p>
		{:else if assets.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><Icon icon={Film01Icon} size={32} /></span>
				<p class="font-medium">No assets yet</p>
				<p class="mt-1 text-sm text-muted">Upload your first video to get started.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each assets as a (a.id)}
					<div class="flex items-center gap-4 py-3">
						<span class="text-muted"><Icon icon={Film01Icon} size={18} /></span>
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm font-medium">{a.filename}</p>
							<p class="mono text-xs text-muted">{bytes(a.bytes)} · {timeAgo(a.created_at)}</p>
						</div>
						<span class="mono text-xs text-muted">{a.status}</span>
						{#if a.status === 'ready'}
							<Button size="sm" variant="secondary" onclick={() => transcode(a.id)}>Transcode</Button>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>
