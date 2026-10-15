package dev.hypernavigator.examples.counter

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.window.ComposeViewport

@OptIn(ExperimentalComposeUiApi::class)
fun main() {
    // Compose state written by the one-way Host bridge and read by the UI below.
    val folderListing = mutableStateOf<FolderListing?>(null)
    HostBridge.subscribeToFolderListing { listing -> folderListing.value = listing }

    ComposeViewport(viewportContainerId = "webApp") {
        CounterApp(folderListing = folderListing.value)
    }
}
