import org.jetbrains.kotlin.gradle.ExperimentalWasmDsl

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)
    // Type-safe Navigation Compose routes are @Serializable classes, as on Android.
    alias(libs.plugins.kotlinSerialization)
}

kotlin {
    @OptIn(ExperimentalWasmDsl::class)
    wasmJs {
        // Fixed output name so index.html can reference a stable script path.
        outputModuleName.set("notes")
        browser {
            commonWebpackConfig {
                outputFileName = "notes.js"
            }
        }
        binaries.executable()
    }

    sourceSets {
        // The UI is Jetpack-Compose-style code in commonMain; only main() lives in wasmJsMain.
        commonMain.dependencies {
            implementation(libs.compose.runtime)
            implementation(libs.compose.foundation)
            implementation(libs.compose.ui)
            implementation(libs.compose.material3)
            implementation(libs.compose.components.resources)
            implementation(libs.androidx.lifecycle.viewmodel.compose)
            implementation(libs.androidx.lifecycle.runtime.compose)
            implementation(libs.androidx.navigation.compose)
        }
    }
}

compose.resources {
    // Generated accessor package for Res.string.* / Res.plurals.* (Android's R equivalent).
    packageOfResClass = "dev.hypernavigator.examples.notes.resources"
}
