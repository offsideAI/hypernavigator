package dev.hypernavigator.examples.notes

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import kotlinx.serialization.Serializable

// Type-safe Navigation Compose routes.
@Serializable
object NotesList

/** [noteId] is null for a new note. */
@Serializable
data class NoteEditor(val noteId: Long? = null)

@Composable
fun NotesApp() {
    // One ViewModel shared by both destinations (Activity-scoped on Android).
    val viewModel = viewModel { NotesViewModel() }
    val navController = rememberNavController()

    MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
        NavHost(navController = navController, startDestination = NotesList) {
            composable<NotesList> {
                NotesListScreen(
                    viewModel = viewModel,
                    onOpenNote = { id -> navController.navigate(NoteEditor(id)) },
                    onNewNote = { navController.navigate(NoteEditor()) },
                )
            }
            composable<NoteEditor> { entry ->
                val route = entry.toRoute<NoteEditor>()
                NoteEditorScreen(
                    viewModel = viewModel,
                    noteId = route.noteId,
                    onDone = { navController.popBackStack() },
                )
            }
        }
    }
}
