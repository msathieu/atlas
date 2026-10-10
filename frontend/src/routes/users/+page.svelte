<script lang="ts">
import { Check, Plus, Trash, X } from "@lucide/svelte";
import { toast } from "svelte-sonner";
import { authenticatedFetch } from "#lib/auth.ts";
import { encryptToServer, formatTime } from "#lib/util.ts";

let { data } = $props();
let selectedUserId = $state<string>();
let selectedUser: any = $derived(data.users.find((user: any) => user.id === selectedUserId));
let dialog: any = $state();
let creationDialog: any = $state();
let confirmationDialog: any = $state();
let confirmationPassword = $state("");
let confirmationAction = $state<((password: string) => Promise<void>) | null>(null);
let confirmationPending = $state(false);
let newUsername = $state("");
let newPassword = $state("");
let newPasswordConfirmation = $state("");
let newAdmin = $state(false);

$effect(() => {
	if (selectedUserId && !selectedUser) {
		dialog?.close();
		selectedUserId = undefined;
	}
});

function confirmPassword(action: (password: string) => Promise<void>) {
	if (confirmationPending) {
		return;
	}
	confirmationAction = action;
	confirmationDialog.showModal();
}
async function submitConfirmation() {
	if (confirmationPending || !confirmationAction) {
		return;
	}
	confirmationPending = true;
	try {
		await confirmationAction(confirmationPassword);
	} catch (_) {
		toast.error("Request Failed");
	} finally {
		confirmationDialog.close();
		confirmationPending = false;
	}
}
function requestCreateUser() {
	if (newPassword !== newPasswordConfirmation) {
		alert("Password fields don't match");
		return;
	}
	const newUser = {
		username: newUsername,
		password: newPassword,
		admin: newAdmin,
	};
	confirmPassword(async (password) => {
		await createUser(newUser, password);
	});
}
async function createUser(newUser: { username: string; password: string; admin: boolean }, password: string) {
	const body = JSON.stringify({ ...newUser, auth_password: password });
	await authenticatedFetch(
		`/api/users`,
		{
			method: "POST",
			body: await encryptToServer(body),
		},
		undefined,
		"application/octet-stream",
	);
	creationDialog.close();
}
async function deleteUser(id: string, password: string) {
	await authenticatedFetch(`/api/users/${id}`, { method: "DELETE", body: await encryptToServer(JSON.stringify({ password })) }, undefined, "application/octet-stream");
	dialog.close();
}
async function setAdmin(id: string, admin: boolean, password: string) {
	await authenticatedFetch(`/api/users/${id}`, { method: "PATCH", body: await encryptToServer(JSON.stringify({ admin, password })) }, undefined, "application/octet-stream");
}
</script>

<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>Admin</th>
			<th>Username</th>
			<th>Email</th>
			<th>Enabled</th>
			<th>Password Enabled</th>
			<th>Last Login</th>
		</tr>
	</thead>
	<tbody>
		{#each data.users as user}
			<tr
				class="cursor-pointer hover:bg-base-200"
				onclick={() => {
					selectedUserId = user.id;
					dialog.showModal();
				}}
			>
				<td>
					{#if user.admin}
						<Check />
					{:else}
						<X />
					{/if}
				</td>
				<td>{user.username}</td>
				<td>{user.email}</td>
				<td>
					{#if user.enabled}
						<Check />
					{:else}
						<X />
					{/if}
				</td>
				<td>
					{#if user.password_enabled}
						<Check />
					{:else}
						<X />
					{/if}
				</td>
				<td>
					{#if user.last_login}
						{formatTime(user.last_login)}
					{:else}
						Never
					{/if}
				</td>
			</tr>
		{/each}
	</tbody>
</table>
<div class="fab">
	<button
		type="button"
		class="btn btn-circle btn-xl btn-accent"
		onclick={() => {
			creationDialog.showModal();
		}}
	>
		<Plus />
	</button>
</div>
<dialog bind:this={dialog} class="modal">
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	{#if selectedUser}
		<div class="modal-box max-w-200">
			<h2 class="text-xl text-center">User</h2>
			<table class="table">
				<tbody>
					<tr>
						<td>ID</td>
						<td>{selectedUser.id}</td>
					</tr>
					<tr>
						<td>Username</td>
						<td>{selectedUser.username}</td>
					</tr>
					<tr>
						<td>Email</td>
						<td>{selectedUser.email}</td>
					</tr>
					<tr>
						<td>Admin</td>
						<td>
							{selectedUser.admin}
							<button
								type="button"
								class="btn btn-sm"
								onclick={() => {
									const { id, admin } = selectedUser;
									confirmPassword((password) => setAdmin(id, !admin, password));
								}}
							>
								Toggle Admin
							</button>
						</td>
					</tr>
					<tr>
						<td>Enabled</td>
						<td>{selectedUser.enabled}</td>
					</tr>
					<tr>
						<td>Password Enabled</td>
						<td>{selectedUser.password_enabled}</td>
					</tr>
					<tr>
						<td>Last Login</td>
						<td>
							{#if selectedUser.last_login}
								{formatTime(selectedUser.last_login)}
							{:else}
								Never
							{/if}
						</td>
					</tr>
				</tbody>
			</table>
			<div class="divider"></div>
			<div>
				<button
					type="button"
					class="btn"
					onclick={() => {
						const { id } = selectedUser;
						confirmPassword((password) => deleteUser(id, password));
					}}
				>
					<Trash />
				</button>
			</div>
		</div>
	{/if}
</dialog>
<dialog
	bind:this={creationDialog}
	class="modal"
	onclose={() => {
		newUsername = "";
		newPassword = "";
		newPasswordConfirmation = "";
		newAdmin = false;
	}}
>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box max-w-400">
		<h2 class="text-xl text-center">Create User</h2>
		<br>
		<form
			onsubmit={(e) => {
				e.preventDefault();
				requestCreateUser();
			}}
		>
			<fieldset class="fieldset">
				<div>
					<label for="username">Username</label>
					<input id="username" type="text" placeholder="Username" class="input" required autocomplete="username" bind:value={newUsername}>
				</div>
				<div>
					<label for="password">Password</label>
					<input id="password" type="password" placeholder="Password" class="input" required autocomplete="new-password" bind:value={newPassword}>
				</div>
				<div>
					<label for="passwordConfirmation">Confirm Password</label>
					<input id="passwordConfirmation" type="password" placeholder="Confirm Password" class="input" required autocomplete="new-password" bind:value={newPasswordConfirmation}>
				</div>
				<div>
					<label for="admin">Admin</label>
					<input id="admin" type="checkbox" class="checkbox" bind:checked={newAdmin}>
				</div>
				<button type="submit" class="btn btn-primary">Create</button>
			</fieldset>
		</form>
	</div>
</dialog>
<dialog
	bind:this={confirmationDialog}
	class="modal"
	onclose={() => {
		confirmationAction = null;
		confirmationPassword = "";
	}}
>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box">
		<h2 class="text-xl text-center">Confirm Password</h2>
		<form
			onsubmit={async (e) => {
				e.preventDefault();
				await submitConfirmation();
			}}
		>
			<fieldset class="fieldset" disabled={confirmationPending}>
				<input id="confirmationPassword" type="password" placeholder="Password" class="input w-full" required autocomplete="current-password" bind:value={confirmationPassword}>
				<button type="submit" class="btn btn-primary">Confirm</button>
			</fieldset>
		</form>
	</div>
</dialog>
