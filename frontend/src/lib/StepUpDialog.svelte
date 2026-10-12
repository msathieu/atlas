<script lang="ts">
import { toast } from "svelte-sonner";
import { authenticatedFetch, getPasskeyVerification } from "./auth.ts";
import { user } from "./state.svelte.ts";
import type { StepUpAction, StepUpVerification } from "./step_up.ts";

let dialog: any = $state();
let password = $state("");
let pending = $state(false);
let action = $state<StepUpAction | null>();

export function confirm(callback: StepUpAction) {
	if (pending) {
		return;
	}
	password = "";
	action = callback;
	dialog.showModal();
}
async function submit(usePasskey: boolean) {
	if (pending || !action) {
		return;
	}
	pending = true;
	try {
		let verification: StepUpVerification;
		if (usePasskey) {
			const response = await authenticatedFetch("/api/passkeys/start_step_up", { method: "POST" });
			verification = { passkey: await getPasskeyVerification(response) };
		} else {
			verification = { password };
		}
		await action(verification);
	} catch {
		toast.error("Request Failed");
	} finally {
		dialog.close();
		pending = false;
	}
}
</script>

<dialog
	bind:this={dialog}
	class="modal"
	onclose={() => {
		action = null;
		password = "";
	}}
>
	<form class="modal-backdrop" method="dialog">
		<button type="submit">Close</button>
	</form>
	<div class="modal-box">
		<h2 class="text-xl text-center">Verify Your Identity</h2>
		{#if user.password_enabled}
			<form
				onsubmit={async (e) => {
					e.preventDefault();
					await submit(false);
				}}
			>
				<fieldset class="fieldset" disabled={pending}>
					<label for="stepUpPassword">Password</label>
					<input id="stepUpPassword" type="password" placeholder="Password" class="input w-full" required autocomplete="current-password" bind:value={password}>
					<button type="submit" class="btn btn-primary">Verify With Password</button>
				</fieldset>
			</form>
			<div class="divider">or</div>
		{/if}
		<button type="button" class="btn btn-primary w-full" disabled={pending} onclick={async () => await submit(true)}>Verify With Passkey</button>
	</div>
</dialog>
