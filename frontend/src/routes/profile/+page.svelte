<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
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
    const response = await authenticatedFetch(`/api/user/update_password`, {
      method: "POST",
      body: JSON.stringify({ old_password: oldPassword, new_password: newPassword }),
    });
    if (response.ok) {
      dialog.close();
      toast.success("Changed password");
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
        <input type="password" placeholder="Old password" class="input" required bind:value={oldPassword} />
        <input type="password" placeholder="New password" class="input" required bind:value={newPassword} />
        <input type="password" placeholder="Confirm password" class="input" required bind:value={passwordConfirmation} />
        <button class="btn btn-primary">Update</button>
      </fieldset>
    </form>
  </div>
</dialog>
