// Behaviour of src-tauri/src/guest_input_shim.js, run against a stand-in MouseEvent class.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';

const shim = readFileSync(join(import.meta.dirname, '..', 'src-tauri/src/guest_input_shim.js'), 'utf8');

function loadShim() {
  class MouseEvent {
    constructor(type, { button = 0, buttons = 0 } = {}) {
      this.type = type;
      this.button = button;
      this._buttons = buttons;
    }
    get buttons() {
      return this._buttons;
    }
  }
  class PointerEvent extends MouseEvent {}
  const context = vm.createContext({ MouseEvent, PointerEvent });
  vm.runInContext(shim, context);
  return { MouseEvent, PointerEvent, context };
}

test('a tap-to-click pointerdown reporting no buttons reads as the pressed button', () => {
  const { PointerEvent, MouseEvent } = loadShim();
  assert.equal(new PointerEvent('pointerdown', { button: 0, buttons: 0 }).buttons, 1);
  assert.equal(new MouseEvent('mousedown', { button: 0, buttons: 0 }).buttons, 1);
  assert.equal(new PointerEvent('pointerdown', { button: 2, buttons: 0 }).buttons, 2);
  assert.equal(new PointerEvent('pointerdown', { button: 1, buttons: 0 }).buttons, 4);
});

test('all other events keep their real buttons value', () => {
  const { PointerEvent } = loadShim();
  assert.equal(new PointerEvent('pointerup', { button: 0, buttons: 0 }).buttons, 0);
  assert.equal(new PointerEvent('pointermove', { button: -1, buttons: 0 }).buttons, 0);
  assert.equal(new PointerEvent('pointerdown', { button: 0, buttons: 1 }).buttons, 1);
  assert.equal(new PointerEvent('pointerdown', { button: 0, buttons: 3 }).buttons, 3);
  assert.equal(new PointerEvent('pointerdown', { button: -1, buttons: 0 }).buttons, 0);
});

test('installing twice does not wrap the getter twice', () => {
  const { MouseEvent, context } = loadShim();
  const first = Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'buttons').get;
  vm.runInContext(shim, context);
  assert.equal(Object.getOwnPropertyDescriptor(MouseEvent.prototype, 'buttons').get, first);
});

test('the Host injects the shim into guest WebViews', () => {
  const guest = readFileSync(join(import.meta.dirname, '..', 'src-tauri/src/guest.rs'), 'utf8');
  assert.match(guest, /include_str!\("guest_input_shim\.js"\)/);
  assert.match(guest, /\.initialization_script\(INPUT_SHIM\)/);
});
