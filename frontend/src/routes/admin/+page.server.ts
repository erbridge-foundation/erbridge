import { backend_internal_url } from '$lib/server/env';
import { listAdminAccounts, listBlocks } from '$lib/api';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const url = backend_internal_url();

	const [accounts, blocks] = await Promise.all([
		listAdminAccounts(fetch, url, cookie),
		listBlocks(fetch, url, cookie)
	]);

	const adminCount = accounts.filter((a) => a.is_server_admin).length;
	return { adminCount, blockCount: blocks.length };
};
