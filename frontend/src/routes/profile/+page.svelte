<script lang="ts">
import { Passkeys } from "@capawesome/capacitor-passkeys";
import { Check, Trash } from "@lucide/svelte";
import { toast } from "svelte-sonner";
import { authenticatedFetch } from "#lib/auth.ts";
import StepUpDialog from "#lib/StepUpDialog.svelte";
import { type StepUpVerification, stepUpFetch } from "#lib/step_up.ts";
import { encryptedFetch, formatTime } from "#lib/util.ts";

let { data } = $props();
let passwordDialog: any = $state();
let confirmation: any = $state();
let newPassword = $state("");
let passwordConfirmation = $state("");
let passwordEnabled = $derived(data.user.password_enabled);
let email = $derived(data.user.email);

function requestPasswordChange() {
	if (newPassword !== passwordConfirmation) {
		alert("New password fields don't match");
		return;
	}
	const intendedPassword = newPassword;
	confirmation.confirm((verification: StepUpVerification) => changePassword(verification, intendedPassword));
}
async function changePassword(verification: StepUpVerification, newPassword: string) {
	await stepUpFetch("/api/user/update_password", "POST", { new_password: newPassword }, verification);
	passwordDialog.close();
	localStorage.removeItem("jwt");
	location.reload();
}
async function setPasswordEnabled(verification: StepUpVerification, enabled: boolean) {
	await stepUpFetch("/api/user/info", "PATCH", { password_state: enabled }, verification);
	toast.success("Changed Password State");
}
async function updateEmail(verification: StepUpVerification, newEmail: string) {
	await stepUpFetch("/api/user/info", "PATCH", { email: newEmail }, verification);
	toast.success("Changed Email");
}
async function registerPasskey(verification: StepUpVerification) {
	const startRes = await stepUpFetch("/api/passkeys/start_registration", "POST", {}, verification);
	const startData = await startRes.json();
	const credential = await Passkeys.createPasskey(startData.publicKey);
	await encryptedFetch(`/api/passkeys/finish_registration`, {
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
			onsubmit={(e) => {
				e.preventDefault();
				requestPasswordChange();
			}}
		>
			<fieldset class="fieldset">
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
<StepUpDialog bind:this={confirmation} />
<br>
<button
	type="button"
	class="btn"
	onclick={() => {
		confirmation.confirm(registerPasskey);
	}}
>
	Register Passkey
</button>
<form
	onsubmit={(e) => {
		e.preventDefault();
		const intendedEnabled = passwordEnabled;
		confirmation.confirm((verification: StepUpVerification) => setPasswordEnabled(verification, intendedEnabled));
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
		confirmation.confirm((verification: StepUpVerification) => updateEmail(verification, intendedEmail));
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
