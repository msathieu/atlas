<script lang="ts">
  import { goto } from "$app/navigation";
  import { API_HOST } from "$lib/util";
  import { connectWS } from "$lib/ws.svelte";
  import { toast } from "svelte-sonner";

  let username = $state("");
  let password = $state("");

  async function login() {
    const response = await fetch(`${API_HOST}/api/login`, {
      method: "POST",
      body: JSON.stringify({ username, password }),
      headers: { "Content-Type": "application/json" },
    });
    if (response.ok) {
      const json = await response.json();
      localStorage.setItem("jwt", json["jwt"]);
      localStorage.setItem("admin", json["admin"]);
      await connectWS();
      await goto("/");
    } else {
      toast.error("Invalid Credentials");
    }
  }
</script>

<div class="flex" style="height: 90%">
  <div class="m-auto">
    <h1 class="text-6xl">
      <img src="/logo.png" alt="logo" class="inline h-32" />Atlas
    </h1>
    <br />
    <form
      onsubmit={async () => {
        await login();
      }}
    >
      <fieldset class="fieldset">
        <input type="text" placeholder="Username" class="input" required bind:value={username} />
        <input type="password" placeholder="Password" class="input" required bind:value={password} />
        <button class="btn btn-primary">Login</button>
      </fieldset>
    </form>
  </div>
</div>
<footer class="footer footer-center">
  <a href="/THIRD-PARTY.txt" class="link link-primary" target="_blank">Third-Party Licenses</a>
</footer>
