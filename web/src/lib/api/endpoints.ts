// Typed API calls. Thin wrappers over apiRequest; components never build URLs.
import { apiRequest, API_BASE } from './client';
import { session } from './session.svelte';
import type { Asset, Job } from './types';

export interface TenantCreated {
	tenant: { id: string; name: string; plan: string };
	api_key: string;
}

export function createTenant(name: string) {
	return apiRequest<TenantCreated>('/v1/tenants', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

export function getMe() {
	return apiRequest<{ id: string; name: string; plan: string }>('/v1/me');
}

interface CreateAssetResponse {
	asset: Asset;
	upload_url: string;
	expires_in_secs: number;
}

export function createAsset(filename: string) {
	return apiRequest<CreateAssetResponse>('/v1/assets', {
		method: 'POST',
		body: JSON.stringify({ filename })
	});
}

export function completeAsset(id: string, bytes: number) {
	return apiRequest<Asset>(`/v1/assets/${id}/complete`, {
		method: 'POST',
		body: JSON.stringify({ bytes })
	});
}

export function listAssets() {
	return apiRequest<Asset[]>('/v1/assets');
}

export function createJob(assetId: string) {
	return apiRequest<Job>('/v1/jobs', {
		method: 'POST',
		body: JSON.stringify({ asset_id: assetId })
	});
}

export interface BatchResult {
	submitted: Job[];
	skipped: { asset_id: string; reason: string }[];
}

export function createJobsBatch(assetIds: string[]) {
	return apiRequest<BatchResult>('/v1/jobs/batch', {
		method: 'POST',
		body: JSON.stringify({ asset_ids: assetIds })
	});
}

export function listJobs() {
	return apiRequest<Job[]>('/v1/jobs');
}

export function getJob(id: string) {
	return apiRequest<Job & { playback_url?: string }>(`/v1/jobs/${id}`);
}

/**
 * Stream a job's status via SSE until it reaches a terminal state.
 * Uses fetch (not EventSource) so the Bearer auth header can be sent.
 * Returns a cleanup function that aborts the stream.
 */
export function streamJob(id: string, onUpdate: (job: Job) => void): () => void {
	const controller = new AbortController();

	(async () => {
		const res = await fetch(`${API_BASE}/v1/jobs/${id}/events`, {
			headers: { Authorization: `Bearer ${session.apiKey}`, Accept: 'text/event-stream' },
			signal: controller.signal
		});
		if (!res.ok || !res.body) return;

		const reader = res.body.getReader();
		const decoder = new TextDecoder();
		let buffer = '';
		for (;;) {
			const { value, done } = await reader.read();
			if (done) break;
			buffer += decoder.decode(value, { stream: true });
			const frames = buffer.split('\n\n');
			buffer = frames.pop() ?? '';
			for (const frame of frames) {
				const data = frame
					.split('\n')
					.find((l) => l.startsWith('data:'))
					?.slice(5)
					.trim();
				if (data) onUpdate(JSON.parse(data) as Job);
			}
		}
	})().catch((e) => {
		if (e.name !== 'AbortError') console.error('[sse]', e);
	});

	return () => controller.abort();
}

/** Upload a file's bytes directly to the presigned storage URL. */
export async function uploadToPresigned(url: string, file: File): Promise<void> {
	const res = await fetch(url, { method: 'PUT', body: file });
	if (!res.ok) throw new Error(`upload failed (${res.status})`);
}
