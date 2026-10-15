// Shows a readable error instead of a blank viewport when the engine lacks WasmGC
// or the Kotlin/Wasm module fails to load.
(function () {
    function showError(message) {
        function render() {
            var el = document.getElementById('wasmError');
            if (!el) return;
            el.textContent = message;
            el.hidden = false;
        }
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', render);
        } else {
            render();
        }
    }

    // Smallest module declaring a WasmGC struct type: (type (struct)).
    var wasmGcProbe = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 3, 1, 95, 0]);
    var supported = typeof WebAssembly === 'object' && WebAssembly.validate(wasmGcProbe);
    if (!supported) {
        showError('This WebView does not support WebAssembly GC, which Kotlin/Wasm requires.\n' +
            'Update macOS / Safari (WebKit with WasmGC, Safari 18.2+).');
        return;
    }

    window.addEventListener('error', function (event) {
        showError('The Compose application failed to start:\n' + (event.message || 'unknown error'));
    });
    window.addEventListener('unhandledrejection', function (event) {
        var reason = event.reason;
        showError('The Compose application failed to start:\n' + ((reason && reason.message) || String(reason)));
    });
})();
