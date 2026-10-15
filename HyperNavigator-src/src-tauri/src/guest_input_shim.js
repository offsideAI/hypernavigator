// Injected by HyperNavigator into every guest page before the page's own scripts (see guest.rs).
//
// On macOS, a trackpad tap-to-click reaches pages in HyperNavigator's WKWebView as a `pointerdown`
// whose `buttons` is 0, because no physical button is ever held. The UI Events spec requires the
// button that went down to be included in `buttons`. Compose Multiplatform for Web decides whether
// the pointer is pressed from `buttons`, so these taps never registered as clicks.
//
// This shim only changes what `buttons` returns for a `pointerdown`/`mousedown` that reports no
// buttons: it returns the bit for `button` instead. Every other event is untouched.
(function () {
    'use strict';
    var descriptor = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'buttons');
    if (!descriptor || typeof descriptor.get !== 'function' || descriptor.get.__hyperNavigatorShim) {
        return;
    }
    var originalGetter = descriptor.get;
    // MouseEvent.button (0 main, 1 auxiliary, 2 secondary, 3 back, 4 forward) -> MouseEvent.buttons bit.
    var BUTTON_BITS = [1, 4, 2, 8, 16];

    var patchedGetter = function () {
        var buttons = originalGetter.call(this);
        if (buttons === 0 && (this.type === 'pointerdown' || this.type === 'mousedown')) {
            var bit = BUTTON_BITS[this.button];
            if (bit) {
                return bit;
            }
        }
        return buttons;
    };
    patchedGetter.__hyperNavigatorShim = true;

    Object.defineProperty(MouseEvent.prototype, 'buttons', {
        get: patchedGetter,
        configurable: true,
        enumerable: descriptor.enumerable,
    });
})();
