<script lang="ts">
import { Passkeys } from "@capawesome/capacitor-passkeys";
import { Check, Trash } from "@lucide/svelte";
import { toast } from "svelte-sonner";
import { authenticatedFetch } from "#lib/auth.ts";
import { encryptToServer, formatTime } from "#lib/util.ts";

let { data } = $props();
let passwordDialog: any = $state();
let confirmationDialog: any = $state();
let confirmationPassword = $state("");
let confirmationAction = $state<((password: string) => Promise<void>) | null>();
let confirmationPending = $state(false);
let oldPassword = $state("");
let newPassword = $state("");
let passwordConfirmation = $state("");
let passwordEnabled = $derived(data.user.password_enabled);
let email = $derived(data.user.email);

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
async function changePassword() {
	if (newPassword !== passwordConfirmation) {
		alert("New password fields don't match");
		return;
	}
	const body = JSON.stringify({
		old_password: oldPassword,
		new_password: newPassword,
	});
	const response = await authenticatedFetch(
		`/api/user/update_password`,
		{
			method: "POST",
			body: await encryptToServer(body),
		},
		undefined,
		"application/octet-stream",
	);
	if (response.ok) {
		passwordDialog.close();
		toast.success("Changed Password");
		localStorage.removeItem("jwt");
		location.reload();
	} else {
		toast.error("Request Denied");
	}
}
async function setPasswordEnabled(password: string, enabled: boolean) {
	const body = JSON.stringify({ password, password_state: enabled });
	const encryptedBody = await encryptToServer(body);
	await authenticatedFetch(
		`/api/user/info`,
		{
			method: "PATCH",
			body: encryptedBody,
		},
		undefined,
		"application/octet-stream",
	);
	toast.success("Changed Password State");
}
async function updateEmail(password: string, newEmail: string) {
	const body = JSON.stringify({ password, email: newEmail });
	const encryptedBody = await encryptToServer(body);
	await authenticatedFetch(
		`/api/user/info`,
		{
			method: "PATCH",
			body: encryptedBody,
		},
		undefined,
		"application/octet-stream",
	);
	toast.success("Changed Email");
}
async function registerPasskey(password: string) {
	const body = JSON.stringify({ password });
	const encryptedBody = await encryptToServer(body);
	const startRes = await authenticatedFetch(`/api/passkeys/start_registration`, { method: "POST", body: encryptedBody }, undefined, "application/octet-stream");
	const startData = await startRes.json();
	const credential = await Passkeys.createPasskey(startData.publicKey);
	await authenticatedFetch(`/api/passkeys/finish_registration`, {
		method: "POST",
		body: JSON.stringify({
			credential,
			name: prompt("Enter a name for your passkey:"),
		}),
	});
}
async function deletePasskey(id: string) {
	await authenticatedFetch(`/api/passkeys/${id}`, {
		method: "DELETE",
	});
}
</script>

<button
	type="button"
	class="btn"
	onclick={() => {
		passwordDialog.showModal();
	}}
>
	Change Password
</button>
<dialog
	bind:this={passwordDialog}
	class="modal"
	onclose={() => {
		oldPassword = "";
		newPassword = "";
		passwordConfirmation = "";
	}}
>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box max-w-200">
		<h1>Change Password</h1>
		<form
			onsubmit={async (e) => {
				e.preventDefault();
				await changePassword();
			}}
		>
			<fieldset class="fieldset">
				<div>
					<label for="oldPassword">Old Password</label>
					<input id="oldPassword" type="password" placeholder="Old password" class="input" required autocomplete="current-password" bind:value={oldPassword}>
				</div>
				<div>
					<label for="newPassword">New Password</label>
					<input id="newPassword" type="password" placeholder="New password" class="input" required autocomplete="new-password" bind:value={newPassword}>
				</div>
				<div>
					<label for="passwordConfirmation">Confirm Password</label>
					<input id="passwordConfirmation" type="password" placeholder="Confirm Password" class="input" required autocomplete="new-password" bind:value={passwordConfirmation}>
				</div>
				<button type="submit" class="btn btn-primary">Update</button>
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
		<br>
		<form
			onsubmit={async (e) => {
				e.preventDefault();
				await submitConfirmation();
			}}
		>
			<fieldset class="fieldset" disabled={confirmationPending}>
				<input id="confirmationPassword" placeholder="Password" type="password" class="input w-full" required autocomplete="current-password" bind:value={confirmationPassword}>
				<button type="submit" class="btn btn-primary">Confirm</button>
			</fieldset>
		</form>
	</div>
</dialog>
<br>
<button
	type="button"
	class="btn"
	onclick={() => {
		confirmPassword(registerPasskey);
	}}
>
	Register Passkey
</button>
<form
	onsubmit={(e) => {
		e.preventDefault();
		const intendedEnabled = passwordEnabled;
		confirmPassword((password) => setPasswordEnabled(password, intendedEnabled));
	}}
>
	<label for="passwordEnabled">Allow Password Login:</label>
	<input id="passwordEnabled" type="checkbox" class="checkbox" bind:checked={passwordEnabled}>
	<button type="submit" class="btn"><Check /></button>
</form>
<form
	onsubmit={(e) => {
		e.preventDefault();
		const intendedEmail = email;
		confirmPassword((password) => updateEmail(password, intendedEmail));
	}}
>
	<label for="email">Email:</label>
	<input id="email" type="email" class="input" required bind:value={email}>
	<button type="submit" class="btn"><Check /></button>
</form>
<div class="divider"></div>
<table class="border border-base-content/10 table">
	<thead>
		<tr>
			<th>Name</th>
			<th>Created At</th>
			<th>Last Used</th>
			<th>Delete</th>
		</tr>
	</thead>
	<tbody>
		{#each data.passkeys as passkey}
			<tr class="hover:bg-base-200">
				<td>{passkey.name}</td>
				<td>{formatTime(passkey.created_at)}</td>
				{#if passkey.last_used}
					<td>{formatTime(passkey.last_used)}</td>
				{:else}
					<td>Never</td>
				{/if}
				<td>
					<button
						type="button"
						class="btn"
						onclick={async () => {
							await deletePasskey(passkey.id);
						}}
					>
						<Trash />
					</button>
				</td>
			</tr>
		{/each}
	</tbody>
</table>
