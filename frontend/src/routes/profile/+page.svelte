<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
  import { encryptToServer } from "$lib/util";
  import { toast } from "svelte-sonner";

  let passwordDialog: any = $state();
  let oldPassword = $state("");
  let newPassword = $state("");
  let passwordConfirmation = $state("");

  async function changePassword() {
    if (newPassword != passwordConfirmation) {
      alert("New password fields don't match");
      return;
    }
    const body = JSON.stringify({ old_password: oldPassword, new_password: newPassword });
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
      toast.success("Changed password");
      localStorage.clear();
      location.reload();
    } else {
      toast.error("Request denied");
    }
  }
  async function registerPasskey() {
    const body = JSON.stringify({ password: prompt("Enter your password to register a new passkey:") });
    const encryptedBody = await encryptToServer(body);
    const startRes = await authenticatedFetch(`/api/passkeys/start_registration`, { method: "POST", body: encryptedBody, headers: { "Content-Type": "application/octet-stream" } });
    const startData = await startRes.json();
    const credential = await navigator.credentials.create({ publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(startData["publicKey"]) });
    await authenticatedFetch(`/api/passkeys/finish_registration`, {
      method: "POST",
      body: JSON.stringify(credential),
    });
  }
</script>

<button
  class="btn"
  onclick={() => {
    passwordDialog.showModal();
  }}
>
  Change Password
</button>
<dialog bind:this={passwordDialog} class="modal">
  <form class="modal-backdrop" method="dialog">
    <button>Close</button>
  </form>
  <div class="modal-box max-w-200">
    <h1>Change Password</h1>
    <form
      onsubmit={async () => {
        await changePassword();
      }}
    >
      <fieldset class="fieldset">
        <input id="oldPassword" type="password" placeholder="Old password" class="input" required autocomplete="current-password" bind:value={oldPassword} />
        <input id="newPassword" type="password" placeholder="New password" class="input" required autocomplete="new-password" bind:value={newPassword} />
        <input id="passwordConfirmation" type="password" placeholder="Confirm password" class="input" required autocomplete="new-password" bind:value={passwordConfirmation} />
        <button class="btn btn-primary">Update</button>
      </fieldset>
    </form>
  </div>
</dialog>
<br />
<button
  class="btn"
  onclick={async () => {
    await registerPasskey();
  }}
>
  Register Passkey
</button>
