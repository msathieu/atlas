import { defineEnvVars } from "@sveltejs/kit/env";

export const variables = defineEnvVars({
	PUBLIC_API_HOST: {
		public: true,
		static: true,
		schema: (value) => value ?? "",
	},
	PUBLIC_API_SIGNING_PUBKEY: {
		public: true,
		static: true,
		schema: (value) => value ?? "",
	},
});
