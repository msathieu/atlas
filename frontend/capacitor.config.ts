import type { CapacitorConfig } from "@capacitor/cli";

const config: CapacitorConfig = {
	appId: "com.strypsteen.atlas",
	appName: "Atlas",
	webDir: "build",
	android: {
		buildOptions: {
			releaseType: "APK",
			signingType: "apksigner",
			keystorePath: "release.keystore",
		},
	},
};

export default config;
