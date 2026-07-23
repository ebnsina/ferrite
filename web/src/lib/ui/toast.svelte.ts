// Lightweight toast notifications. Import `toast` anywhere and call
// toast.success / .error / .info; render the stack once via <Toaster/>.
export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
	id: number;
	kind: ToastKind;
	message: string;
}

let items = $state<Toast[]>([]);
let seq = 0;

// Errors linger a little longer than confirmations so they can be read.
const DEFAULT_TTL: Record<ToastKind, number> = {
	success: 3500,
	info: 4000,
	error: 6000
};

function push(kind: ToastKind, message: string, ttl?: number): number {
	const id = ++seq;
	items = [...items, { id, kind, message }];
	const life = ttl ?? DEFAULT_TTL[kind];
	if (life > 0 && typeof window !== 'undefined') {
		setTimeout(() => dismiss(id), life);
	}
	return id;
}

function dismiss(id: number) {
	items = items.filter((t) => t.id !== id);
}

export const toast = {
	get items() {
		return items;
	},
	success: (message: string, ttl?: number) => push('success', message, ttl),
	error: (message: string, ttl?: number) => push('error', message, ttl),
	info: (message: string, ttl?: number) => push('info', message, ttl),
	dismiss
};
