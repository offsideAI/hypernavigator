package dev.hypernavigator.examples.notes

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

data class Note(val id: Long, val title: String, val body: String)

/** Holds the notes for the whole app, exactly as an Activity-scoped ViewModel would on Android. */
class NotesViewModel : ViewModel() {
    private val _notes = MutableStateFlow(
        listOf(
            Note(1, "Welcome", "This screen uses Material 3, a ViewModel, Navigation Compose and string resources."),
            Note(2, "Same code as Android", "Everything in commonMain is written the way you would write it in an Android app."),
            Note(3, "Try it", "Tap a note to edit it, or add a new one."),
        ),
    )
    val notes: StateFlow<List<Note>> = _notes.asStateFlow()

    private var nextId = 4L

    fun note(id: Long): Note? = _notes.value.firstOrNull { it.id == id }

    /** Creates a note when [id] is null, otherwise updates it. */
    fun save(id: Long?, title: String, body: String) {
        _notes.update { notes ->
            if (id == null) {
                listOf(Note(nextId++, title.trim(), body.trim())) + notes
            } else {
                notes.map { if (it.id == id) it.copy(title = title.trim(), body = body.trim()) else it }
            }
        }
    }

    fun delete(id: Long) {
        _notes.update { notes -> notes.filterNot { it.id == id } }
    }
}
