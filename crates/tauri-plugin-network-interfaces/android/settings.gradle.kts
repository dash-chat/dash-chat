pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    plugins {
        id("com.android.library") version "8.11.0"
        id("org.jetbrains.kotlin.android") version "1.9.25"
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "tauri-plugin-network-interfaces"
include(":tauri-android")
project(":tauri-android").projectDir = file(".tauri/tauri-api")
