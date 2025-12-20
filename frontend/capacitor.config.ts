import { CapacitorConfig } from "@capacitor/cli";

const config: CapacitorConfig = {
  appId: "com.strypsteen.atlas",
  appName: "Atlas",
  webDir: "build",
  plugins: {
    CapacitorHttp: {
      enabled: true,
    },
  },
  android: {
    buildOptions: {
      releaseType: "APK",
      signingType: "apksigner",
      keystorePath: "release.keystore",
    },
  },
};

export default config;
