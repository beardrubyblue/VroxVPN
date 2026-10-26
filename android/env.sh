# Окружение Android-сборки (источник: source android/env.sh). JDK 17 и
# Android SDK/NDK — Homebrew openjdk@17 + sdkmanager (~/Library/Android/sdk),
# см. docs/ARCHITECTURE.md, раздел про Android. Переопределяются снаружи.
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
export ANDROID_NDK_HOME="$NDK_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"
