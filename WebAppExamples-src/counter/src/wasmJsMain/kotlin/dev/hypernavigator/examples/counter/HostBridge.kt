@file:OptIn(ExperimentalWasmJsInterop::class)

package dev.hypernavigator.examples.counter

data class FolderEntry(val name: String, val kind: String)

data class FolderListing(val entries: List<FolderEntry>, val truncated: Boolean)

/**
 * Receives the one-way, Host-initiated folder listing event.
 *
 * HyperNavigator dispatches `hypernavigator:folder-listing` on `window` with `detail` set to a JSON
 * string (protocol version 1). The guest has no way to call back into the Host.
 */
object HostBridge {
    private const val PROTOCOL_VERSION = 1
    private const val MAX_ENTRIES = 100
    private const val MAX_NAME_CHARS = 255
    private val KNOWN_KINDS = setOf("file", "directory", "symlink", "other")

    fun subscribeToFolderListing(onListing: (FolderListing) -> Unit) {
        subscribe { message -> parse(message)?.let(onListing) }
    }

    private fun parse(message: JsAny): FolderListing? {
        if (messageType(message) != "hypernavigator.folderListing") return null
        if (messageVersion(message) != PROTOCOL_VERSION) return null
        val count = entryCount(message).coerceIn(0, MAX_ENTRIES)
        val entries = (0 until count).mapNotNull { i ->
            val name = entryName(message, i).take(MAX_NAME_CHARS)
            val kind = entryKind(message, i)
            if (name.isEmpty() || kind !in KNOWN_KINDS) null else FolderEntry(name, kind)
        }
        return FolderListing(entries, isTruncated(message))
    }
}

// The listener parses `detail` with JSON.parse; it never evaluates it.
private fun subscribe(callback: (JsAny) -> Unit): Unit = js(
    """{
    window.addEventListener('hypernavigator:folder-listing', function (event) {
        var detail = event.detail;
        if (typeof detail !== 'string' || detail.length > 262144) return;
        var parsed;
        try { parsed = JSON.parse(detail); } catch (e) { return; }
        if (parsed === null || typeof parsed !== 'object') return;
        callback(parsed);
    });
}"""
)

private fun messageType(message: JsAny): String = js("(typeof message.type === 'string') ? message.type : ''")

private fun messageVersion(message: JsAny): Int = js("(typeof message.version === 'number') ? (message.version | 0) : -1")

private fun isTruncated(message: JsAny): Boolean = js("message.truncated === true")

private fun entryCount(message: JsAny): Int = js("Array.isArray(message.entries) ? message.entries.length : 0")

private fun entryName(message: JsAny, index: Int): String =
    js("(message.entries[index] && typeof message.entries[index].name === 'string') ? message.entries[index].name : ''")

private fun entryKind(message: JsAny, index: Int): String =
    js("(message.entries[index] && typeof message.entries[index].kind === 'string') ? message.entries[index].kind : ''")
