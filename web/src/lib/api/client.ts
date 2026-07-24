// Thin typed client over the Ferrite Stream API.
//
// Every failure path is modelled: non-2xx responses become `ApiError` with the
// server's code/message; network failures and non-JSON bodies degrade to a
// generic ApiError rather than throwing something unhandled. Callers can rely
// on catching `ApiError` and reading `.status` / `.code`.

import type { ApiErrorBody } from './types';
import { session } from './session.svelte';
import { PUBLIC_API_URL } from '$env/static/public';

/** API base URL. Required — build fails if PUBLIC_API_URL is unset. */
export const API_BASE = PUBLIC_API_URL;

const CSRF_COOKIE = 'ferrite_csrf';
const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

/** Read the CSRF token the API set as a (non-HttpOnly) cookie. */
function csrfToken(): string | null {
	if (typeof document === 'undefined') return null;
	const match = document.cookie.match(new RegExp(`(?:^|; )${CSRF_COOKIE}=([^;]*)`));
	return match ? decodeURIComponent(match[1]) : null;
}

export class ApiError extends Error {
	readonly status: number;
	readonly code: string;
	readonly fields?: unknown;

	constructor(status: number, code: string, message: string, fields?: unknown) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.code = code;
		this.fields = fields;
	}

	get isNotFound() {
		return this.status === 404;
	}
	get isUnauthorized() {
		return this.status === 401;
	}
	get isServerError() {
		return this.status >= 500;
	}
}

export interface RequestOptions extends RequestInit {
	/** Optional fetch implementation (pass SvelteKit's `fetch` in load funcs). */
	fetch?: typeof fetch;
}

export async function apiRequest<T>(path: string, opts: RequestOptions = {}): Promise<T> {
	const { fetch: fetchImpl = fetch, headers, ...rest } = opts;
	const url = path.startsWith('http') ? path : `${API_BASE}${path}`;

	// Auth rides in the HttpOnly session cookie (credentials: 'include'); state-
	// changing requests add the double-submit CSRF header the API validates.
	const method = (rest.method ?? 'GET').toUpperCase();
	const csrf = SAFE_METHODS.has(method) ? null : csrfToken();

	let res: Response;
	try {
		res = await fetchImpl(url, {
			...rest,
			credentials: 'include',
			headers: {
				Accept: 'application/json',
				...(rest.body ? { 'Content-Type': 'application/json' } : {}),
				...(csrf ? { 'X-CSRF-Token': csrf } : {}),
				...headers
			}
		});
	} catch (cause) {
		// DNS failure, offline, CORS, aborted — never surfaces as an unhandled reject.
		throw new ApiError(0, 'network_error', 'Could not reach the server.', cause);
	}

	if (res.status === 204) {
		return undefined as T;
	}

	const raw = await res.text();
	const parsed = raw ? safeJsonParse(raw) : undefined;

	if (!res.ok) {
		// A rejected session (expired/cleared cookie, or stale local identity from
		// before cookie auth) drops us back to the sign-in screen.
		if (res.status === 401) session.clear();
		const body = parsed as ApiErrorBody | undefined;
		throw new ApiError(
			res.status,
			body?.error?.code ?? 'http_error',
			body?.error?.message ?? `Request failed (${res.status})`,
			body?.error?.fields
		);
	}

	return parsed as T;
}

function safeJsonParse(raw: string): unknown {
	try {
		return JSON.parse(raw);
	} catch {
		return undefined;
	}
}
