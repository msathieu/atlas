import { defineEnvVars } from "@sveltejs/kit/env";

export const variables = defineEnvVars({
  PUBLIC_API_ENCRYPTION_PUBKEY: { public: true, static: true },
  PUBLIC_API_HOST: { public: true, static: true },
  PUBLIC_API_SIGNING_PUBKEY: { public: true, static: true },
});
