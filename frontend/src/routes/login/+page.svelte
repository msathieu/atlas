<script lang="ts">
  import { goto } from "$app/navigation";
  import { verifyResponseSignature } from "$lib/auth.ts";
  import { API_HOST, bufferToBase64, encryptToServer } from "$lib/util.ts";
  import { toast } from "svelte-sonner";

  let username = $state("");
  let password = $state("");

  async function login() {
    const keyPair = await crypto.subtle.generateKey("Ed25519", true, ["sign", "verify"]);
    const pubkey = bufferToBase64(await crypto.subtle.exportKey("raw", keyPair.publicKey));
    const body = JSON.stringify({ username, password, pubkey, timestamp: Math.floor(Date.now() / 1000) });
    const encryptedBody = await encryptToServer(body);
    const response = await fetch(`${API_HOST}/api/login`, {
      method: "POST",
      body: encryptedBody,
      headers: { "Content-Type": "application/octet-stream" },
    });
    const bodyHash = await crypto.subtle.digest("SHA-512", encryptedBody);
    await verifyResponseSignature(response, `login-${bufferToBase64(bodyHash)}`);
    if (response.ok) {
      const json = await response.json();
      localStorage.setItem("jwt", json["jwt"]);
      localStorage.setItem("privkey", bufferToBase64(await crypto.subtle.exportKey("pkcs8", keyPair.privateKey)));
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
        <input id="username" type="text" placeholder="Username" class="input" required autocomplete="username" bind:value={username} />
        <input id="password" type="password" placeholder="Password" class="input" required autocomplete="current-password" bind:value={password} />
        <button class="btn btn-primary">Login</button>
      </fieldset>
    </form>
  </div>
</div>
<footer class="footer footer-center">
  <a href="/THIRD-PARTY.txt" class="link link-primary" target="_blank">Third-Party Licenses</a>
</footer>
