package dev.hypernavigator.examples.notes

import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.window.ComposeViewport

// The only HyperNavigator-specific code. On Android this would be:
//   class MainActivity : ComponentActivity() {
//       override fun onCreate(savedInstanceState: Bundle?) {
//           super.onCreate(savedInstanceState)
//           setContent { NotesApp() }
//       }
//   }
@OptIn(ExperimentalComposeUiApi::class)
fun main() {
    ComposeViewport(viewportContainerId = "webApp") {
        NotesApp()
    }
}
