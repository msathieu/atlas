<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
  import { encryptToServer } from "$lib/util";
  import { toast } from "svelte-sonner";

  let dialog: any = $state();
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
      dialog.close();
      toast.success("Changed password");
      localStorage.clear();
      location.reload();
    } else {
      toast.error("Request denied");
    }
  }
</script>

<button
  class="btn"
  onclick={() => {
    dialog.showModal();
  }}
>
  Change password
</button>
<dialog bind:this={dialog} class="modal">
  <form class="modal-backdrop" method="dialog">
    <button>Close</button>
  </form>
  <div class="modal-box max-w-200">
    <h1>Change password</h1>
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
