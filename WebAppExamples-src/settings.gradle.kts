rootProject.name = "webapp-examples"

pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
        google()
    }
}

// One module per example app; each builds to <module>/build/dist/wasmJs/productionExecutable/.
include(":counter")
include(":notes")
