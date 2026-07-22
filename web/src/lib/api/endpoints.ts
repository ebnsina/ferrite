// Typed API calls. Thin wrappers over apiRequest; components never build URLs.
import { apiRequest } from './client';
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

export function listJobs() {
	return apiRequest<Job[]>('/v1/jobs');
}

export function getJob(id: string) {
	return apiRequest<Job & { playback_url?: string }>(`/v1/jobs/${id}`);
}

/** Upload a file's bytes directly to the presigned storage URL. */
export async function uploadToPresigned(url: string, file: File): Promise<void> {
	const res = await fetch(url, { method: 'PUT', body: file });
	if (!res.ok) throw new Error(`upload failed (${res.status})`);
}
