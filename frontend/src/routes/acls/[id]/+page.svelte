<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import MemberPicker from '$lib/components/MemberPicker.svelte';
	import { permissionsFor, type MemberType } from '$lib/acl-permissions';
	import type { AclMemberDto, EntityCharacterDto, EntityOrgDto } from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	type FormShape = {
		action: string;
		code?: string;
		message?: string;
		memberId?: string;
		query?: string;
		characters?: EntityCharacterDto[];
		corporations?: EntityOrgDto[];
		alliances?: EntityOrgDto[];
		unavailable?: boolean;
	};
	let f = $derived(form as FormShape | null);

	// Picker state, sourced from the `search` action result.
	let searched = $derived(f?.action === 'search' && !f.code);
	let searchError = $derived(f?.action === 'search' && f.code ? (f.message ?? null) : null);

	// Add errors surface above the picker (the add forms live inside it now).
	let addError = $derived(f?.action === 'addMember' && f.code ? f.message : null);

	// Per-member update permission (inline select). updateError is keyed by member.
	let updateError = $derived(
		f?.action === 'updateMember' && f.code ? { memberId: f.memberId, message: f.message } : null
	);
	let removeError = $derived(
		f?.action === 'removeMember' && f.code ? { memberId: f.memberId, message: f.message } : null
	);

	let removeState = $state<{ open: boolean; member: AclMemberDto | null }>({
		open: false,
		member: null
	});
	let removeForms = $state<Record<string, HTMLFormElement>>({});

	function memberTypeLabel(t: string): string {
		if (t === 'character') return m.acl_member_type_character();
		if (t === 'corporation') return m.acl_member_type_corporation();
		if (t === 'alliance') return m.acl_member_type_alliance();
		return t;
	}

	function permLabel(p: string): string {
		switch (p) {
			case 'read':
				return m.acl_perm_read();
			case 'read_write':
				return m.acl_perm_read_write();
			case 'manage':
				return m.acl_perm_manage();
			case 'admin':
				return m.acl_perm_admin();
			case 'deny':
				return m.acl_perm_deny();
			default:
				return p;
		}
	}
</script>

<svelte:head>
	<title>{data.acl.name}</title>
</svelte:head>

<main class="body">
	<div class="wrap">
		<a class="back" href="/acls">{m.acl_detail_back()}</a>
		<h1 class="acl-title">{data.acl.name}</h1>

		<section class="panel">
			<h2 class="panel-heading">{m.acl_detail_members_heading()}</h2>
			{#if data.members.length === 0}
				<p class="empty" role="status">{m.acl_detail_no_members()}</p>
			{:else}
				<table class="member-table">
					<thead>
						<tr>
							<th>{m.acl_col_type()}</th>
							<th>{m.acl_col_name()}</th>
							<th>{m.acl_col_permission()}</th>
							<th class="actions-col">{m.acl_col_actions()}</th>
						</tr>
					</thead>
					<tbody>
						{#each data.members as member (member.id)}
							<tr>
								<td class="muted">{memberTypeLabel(member.member_type)}</td>
								<td>{member.name}</td>
								<td>
									<form
										method="POST"
										action="?/updateMember"
										use:enhance
										class="perm-form"
									>
										<input type="hidden" name="member_id" value={member.id} />
										<select
											name="permission"
											value={member.permission}
											aria-label={m.acl_update_permission_aria({ name: member.name })}
											onchange={(e) => e.currentTarget.form?.requestSubmit()}
										>
											{#each permissionsFor(member.member_type as MemberType) as p (p)}
												<option value={p}>{permLabel(p)}</option>
											{/each}
										</select>
									</form>
									{#if updateError?.memberId === member.id}
										<p class="inline-error" role="alert">{updateError.message}</p>
									{/if}
								</td>
								<td class="actions-col">
									<form
										bind:this={removeForms[member.id]}
										method="POST"
										action="?/removeMember"
										use:enhance
									>
										<input type="hidden" name="member_id" value={member.id} />
										<button
											type="button"
											class="danger"
											onclick={() => (removeState = { open: true, member })}
										>
											{m.acl_member_remove()}
										</button>
									</form>
									{#if removeError?.memberId === member.id}
										<p class="inline-error" role="alert">{removeError.message}</p>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</section>

		<section class="panel">
			<h2 class="panel-heading">{m.acl_detail_add_heading()}</h2>

			{#if addError}
				<p class="inline-error" role="alert">{addError}</p>
			{/if}

			<MemberPicker
				characters={f?.action === 'search' ? (f.characters ?? []) : []}
				corporations={f?.action === 'search' ? (f.corporations ?? []) : []}
				alliances={f?.action === 'search' ? (f.alliances ?? []) : []}
				unavailable={f?.action === 'search' ? Boolean(f.unavailable) : false}
				{searched}
				errorMessage={searchError}
			/>
		</section>
	</div>
</main>

<ConfirmDialog
	open={removeState.open}
	tone="danger"
	onCancel={() => (removeState = { open: false, member: null })}
	onConfirm={() => {
		if (removeState.member) {
			removeForms[removeState.member.id]?.requestSubmit();
		}
		removeState = { open: false, member: null };
	}}
>
	{#snippet title()}{m.acl_member_remove_title({ name: removeState.member?.name ?? '' })}{/snippet}
	{#snippet body()}{m.acl_member_remove_body()}{/snippet}
	{#snippet confirmLabel()}{m.acl_member_remove_confirm()}{/snippet}
</ConfirmDialog>

<style>
	.body {
		flex: 1;
		overflow: auto;
		padding: 24px;
	}
	.wrap {
		max-width: 760px;
		margin: 0 auto;
	}

	.back {
		display: inline-block;
		margin-bottom: 12px;
		font-size: 0.75rem;
		color: var(--slate-400);
		text-decoration: none;
	}
	.back:hover {
		color: var(--slate-200);
	}

	.acl-title {
		margin: 0;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--slate-100);
	}

	.panel {
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		padding: 20px;
		margin-top: 24px;
	}
	.panel-heading {
		margin: 0 0 12px;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}

	.member-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}
	.member-table th {
		text-align: left;
		padding: 8px 12px;
		color: var(--slate-500);
		font-weight: 500;
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border-bottom: 1px solid var(--space-700);
	}
	.member-table td {
		padding: 10px 12px;
		color: var(--slate-200);
		border-bottom: 1px solid var(--space-800);
		vertical-align: top;
	}
	.actions-col {
		text-align: right;
		width: 1%;
		white-space: nowrap;
	}
	.muted {
		color: var(--slate-500);
	}

	.perm-form select {
		padding: 6px 10px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}

	button.danger {
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: 0.75rem;
		color: var(--slate-400);
		cursor: pointer;
	}
	button.danger:hover {
		color: var(--red);
	}

	.empty {
		padding: 16px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}

	.inline-error {
		margin: 4px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}
</style>
