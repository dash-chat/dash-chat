{ inputs, ... }:

{
  perSystem =
    { lib, pkgs, system, ... }:
    let
      # The Android SDK is unfree and needs its license accepted, so it gets
      # its own nixpkgs import instead of the shared one.
      androidPkgs = import inputs.nixpkgs {
        inherit system;
        config = {
          allowUnfree = true;
          android_sdk.accept_license = true;
        };
      };
      # The Android Gradle plugin installs the build-tools it defaults to and
      # refuses to build when the read-only nix SDK lacks them.
      agpDefaultBuildToolsVersion = "35.0.0";
      aapt2BuildToolsVersion = "37.0.0";
      androidSdk =
        (androidPkgs.androidenv.composeAndroidPackages {
          # Every Tauri plugin's Android module compiles against the platform
          # it declares itself, so the SDK carries each one in use, not only
          # the app's compileSdk.
          platformVersions = [
            "34"
            "35"
            "36"
            "37.0"
          ];
          buildToolsVersions = [
            agpDefaultBuildToolsVersion
            aapt2BuildToolsVersion
          ];
          includeNDK = true;
          ndkVersion = "28.1.13356709";
        }).androidsdk;

      sdkPath = "${androidSdk}/libexec/android-sdk";
      ndkPath = "${sdkPath}/ndk-bundle";
      # The NDK ships one host toolchain per OS, named x86_64 on arm64 macs too.
      ndkHost = "${if pkgs.stdenv.isLinux then "linux" else "darwin"}-x86_64";
      prebuiltPath = "${ndkPath}/toolchains/llvm/prebuilt/${ndkHost}";
      toolchainBinsPath = "${prebuiltPath}/bin";

      ndkApiLevel = "24";
      rustTargets = [
        {
          rustTriple = "aarch64-linux-android";
          clangTriple = "aarch64-linux-android";
          sysrootIncludeDir = "aarch64-linux-android";
        }
        {
          rustTriple = "armv7-linux-androideabi";
          clangTriple = "armv7a-linux-androideabi";
          sysrootIncludeDir = "arm-linux-androideabi";
        }
        {
          rustTriple = "i686-linux-android";
          clangTriple = "i686-linux-android";
          sysrootIncludeDir = "i686-linux-android";
        }
        {
          rustTriple = "x86_64-linux-android";
          clangTriple = "x86_64-linux-android";
          sysrootIncludeDir = "x86_64-linux-android";
        }
      ];
      crossCompileEnv =
        {
          rustTriple,
          clangTriple,
          sysrootIncludeDir,
        }:
        let
          envSuffix = lib.replaceStrings [ "-" ] [ "_" ] rustTriple;
          clang = "${toolchainBinsPath}/${clangTriple}${ndkApiLevel}-clang";
        in
        ''
          export CC_${envSuffix}=${clang}
          export CXX_${envSuffix}=${clang}++
          export AWS_LC_SYS_CXX_${envSuffix}=${clang}++
          export CARGO_TARGET_${lib.toUpper envSuffix}_LINKER=${clang}
          export CFLAGS_${envSuffix}="--target=${clangTriple}${ndkApiLevel} --sysroot=${prebuiltPath}/sysroot"
          export CXXFLAGS_${envSuffix}="--target=${clangTriple}${ndkApiLevel}"
          export BINDGEN_EXTRA_CLANG_ARGS_${envSuffix}="--sysroot=${prebuiltPath}/sysroot -I${prebuiltPath}/sysroot/usr/include/${sysrootIncludeDir}"
        '';
    in
    {
      packages.android-sdk = androidSdk;

      devShells.androidSdk = pkgs.mkShell {
        packages = [
          androidSdk
          pkgs.gradle
          pkgs.jdk17
          pkgs.aapt
        ];

        shellHook = ''
          # The aapt2 Gradle downloads from Maven is a generic-linux binary
          # that cannot run on NixOS.
          export GRADLE_OPTS="-Dorg.gradle.project.android.aapt2FromMavenOverride=${sdkPath}/build-tools/${aapt2BuildToolsVersion}/aapt2"
          export GRADLE_HOME=${pkgs.gradle}

          export ANDROID_HOME=${sdkPath}
          export ANDROID_SDK_ROOT=${sdkPath}
          export ANDROID_NDK=${ndkPath}
          export ANDROID_NDK_HOME=${ndkPath}
          export ANDROID_NDK_ROOT=${ndkPath}
          export NDK_HOME=${ndkPath}
          export ANDROID_NDK_LATEST_HOME=${ndkPath}

          export RANLIB=${toolchainBinsPath}/llvm-ranlib
          export AR=${toolchainBinsPath}/llvm-ar
          unset CC
          unset CXX
          export CLANG_PATH=${toolchainBinsPath}/clang
          export LIBCLANG_PATH=${prebuiltPath}/lib
        ''
        + lib.concatMapStrings crossCompileEnv rustTargets;
      };
    };
}
