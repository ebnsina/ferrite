// Typed API calls. Thin wrappers over apiRequest; components never build URLs.
import { apiRequest, API_BASE } from './client';
import { session } from './session.svelte';
import type {
	ApiKey,
	Asset,
	AuthResponse,
	Job,
	LiveStream,
	Member,
	MemberInvited,
	Usage,
	User
} from './types';

// --- Auth --------------------------------------------------------------------

export function signup(email: string, password: string, workspace: string) {
	return apiRequest<AuthResponse>('/v1/auth/signup', {
		method: 'POST',
		body: JSON.stringify({ email, password, workspace })
	});
}

export function login(email: string, password: string) {
	return apiRequest<AuthResponse>('/v1/auth/login', {
		method: 'POST',
		body: JSON.stringify({ email, password })
	});
}

export function forgotPassword(email: string) {
	return apiRequest<void>('/v1/auth/forgot-password', {
		method: 'POST',
		body: JSON.stringify({ email })
	});
}

export function resetPassword(token: string, new_password: string) {
	return apiRequest<void>('/v1/auth/reset-password', {
		method: 'POST',
		body: JSON.stringify({ token, new_password })
	});
}

// --- Team --------------------------------------------------------------------

export function listMembers() {
	return apiRequest<Member[]>('/v1/members');
}

export function inviteMember(email: string, role: 'admin' | 'member') {
	return apiRequest<MemberInvited>('/v1/members', {
		method: 'POST',
		body: JSON.stringify({ email, role })
	});
}

export function updateMemberRole(id: string, role: 'admin' | 'member') {
	return apiRequest<Member>(`/v1/members/${id}`, {
		method: 'PATCH',
		body: JSON.stringify({ role })
	});
}

export function removeMember(id: string) {
	return apiRequest<void>(`/v1/members/${id}`, { method: 'DELETE' });
}

// --- Profile -----------------------------------------------------------------

export function updateProfile(name: string) {
	return apiRequest<User>('/v1/profile', {
		method: 'PATCH',
		body: JSON.stringify({ name })
	});
}

export function changePassword(current_password: string, new_password: string) {
	return apiRequest<void>('/v1/profile/password', {
		method: 'POST',
		body: JSON.stringify({ current_password, new_password })
	});
}

export function createApiKey(name: string) {
	return apiRequest<{ id: string; prefix: string; api_key: string }>('/v1/api-keys', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

export function listApiKeys() {
	return apiRequest<ApiKey[]>('/v1/api-keys');
}

export function revokeApiKey(id: string) {
	return apiRequest<void>(`/v1/api-keys/${id}`, { method: 'DELETE' });
}

export function getUsage() {
	return apiRequest<Usage>('/v1/usage');
}

export function createLiveStream(name: string) {
	return apiRequest<LiveStream>('/v1/live/streams', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

export function listLiveStreams() {
	return apiRequest<LiveStream[]>('/v1/live/streams');
}

export function getLiveStream(id: string) {
	return apiRequest<LiveStream>(`/v1/live/streams/${id}`);
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

export function clipAsset(id: string, start: number, end: number, name?: string) {
	return apiRequest<{ asset: Asset; job_id: string }>(`/v1/assets/${id}/clip`, {
		method: 'POST',
		body: JSON.stringify({ start, end, name: name || null })
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

export interface TranscodeOptions {
	encrypt?: boolean;
	mp4?: boolean;
	audio?: boolean;
	captions?: boolean;
	watermark?: { position: 'tl' | 'tr' | 'bl' | 'br'; opacity: number };
}

export function createJob(assetId: string, options: TranscodeOptions = {}) {
	return apiRequest<Job>('/v1/jobs', {
		method: 'POST',
		body: JSON.stringify({ asset_id: assetId, ...options })
	});
}

export function getBrand() {
	return apiRequest<{ logo_url: string | null }>('/v1/brand');
}

export function uploadBrandLogo() {
	return apiRequest<{ upload_url: string; expires_in_secs: number }>('/v1/brand/logo', {
		method: 'POST'
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
			headers: { Authorization: `Bearer ${session.token}`, Accept: 'text/event-stream' },
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
