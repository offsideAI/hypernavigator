package dev.hypernavigator.examples.notes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.hypernavigator.examples.notes.resources.Res
import dev.hypernavigator.examples.notes.resources.app_name
import dev.hypernavigator.examples.notes.resources.back
import dev.hypernavigator.examples.notes.resources.body_label
import dev.hypernavigator.examples.notes.resources.delete
import dev.hypernavigator.examples.notes.resources.edit_note
import dev.hypernavigator.examples.notes.resources.empty_state
import dev.hypernavigator.examples.notes.resources.new_note
import dev.hypernavigator.examples.notes.resources.note_count
import dev.hypernavigator.examples.notes.resources.save
import dev.hypernavigator.examples.notes.resources.subtitle
import dev.hypernavigator.examples.notes.resources.title_label
import dev.hypernavigator.examples.notes.resources.untitled
import org.jetbrains.compose.resources.pluralStringResource
import org.jetbrains.compose.resources.stringResource

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotesListScreen(viewModel: NotesViewModel, onOpenNote: (Long) -> Unit, onNewNote: () -> Unit) {
    val notes by viewModel.notes.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(stringResource(Res.string.app_name))
                        Text(
                            pluralStringResource(Res.plurals.note_count, notes.size, notes.size),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                text = { Text(stringResource(Res.string.new_note)) },
                icon = { Text("+", style = MaterialTheme.typography.titleLarge) },
                onClick = onNewNote,
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item {
                Text(
                    stringResource(Res.string.subtitle),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (notes.isEmpty()) {
                item { Text(stringResource(Res.string.empty_state), modifier = Modifier.padding(top = 32.dp)) }
            }
            items(notes, key = { it.id }) { note ->
                NoteCard(note = note, onClick = { onOpenNote(note.id) })
            }
        }
    }
}

@Composable
private fun NoteCard(note: Note, onClick: () -> Unit) {
    Card(onClick = onClick, modifier = Modifier.widthIn(max = 640.dp).fillMaxWidth()) {
        Column(Modifier.padding(16.dp)) {
            Text(
                note.title.ifBlank { stringResource(Res.string.untitled) },
                style = MaterialTheme.typography.titleMedium,
            )
            if (note.body.isNotBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    note.body,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteEditorScreen(viewModel: NotesViewModel, noteId: Long?, onDone: () -> Unit) {
    val existing = noteId?.let(viewModel::note)
    var title by rememberSaveable { mutableStateOf(existing?.title ?: "") }
    var body by rememberSaveable { mutableStateOf(existing?.body ?: "") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(if (existing == null) Res.string.new_note else Res.string.edit_note)) },
                navigationIcon = {
                    TextButton(onClick = onDone) { Text(stringResource(Res.string.back)) }
                },
                actions = {
                    if (existing != null) {
                        TextButton(onClick = { viewModel.delete(existing.id); onDone() }) {
                            Text(stringResource(Res.string.delete), color = MaterialTheme.colorScheme.error)
                        }
                    }
                    TextButton(
                        onClick = { viewModel.save(existing?.id, title, body); onDone() },
                        enabled = title.isNotBlank() || body.isNotBlank(),
                    ) {
                        Text(stringResource(Res.string.save))
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(Modifier.widthIn(max = 640.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    label = { Text(stringResource(Res.string.title_label)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = body,
                    onValueChange = { body = it },
                    label = { Text(stringResource(Res.string.body_label)) },
                    minLines = 6,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}
