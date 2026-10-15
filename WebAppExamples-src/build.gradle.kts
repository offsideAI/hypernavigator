// Root build for the example apps HyperNavigator loads. Plugin versions are pinned in
// gradle/libs.versions.toml and applied per example module.
plugins {
    alias(libs.plugins.kotlinMultiplatform) apply false
    alias(libs.plugins.composeMultiplatform) apply false
    alias(libs.plugins.composeCompiler) apply false
    alias(libs.plugins.kotlinSerialization) apply false
}
