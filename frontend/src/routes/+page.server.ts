import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ parent }) => {
	const { me } = await parent();
	return { me };
};
