import "@testing-library/jest-dom";

// Node.js 26 sets globalThis.localStorage = undefined (experimental API, not configured).
// vitest's populateGlobal skips localStorage because it already exists on global.
// Restore jsdom's localStorage onto globalThis so all modules see a working Storage.
//
// globalThis.jsdom is injected by vitest's jsdom environment setup before setupFiles run.
const jsdomWindow = (globalThis as unknown as { jsdom?: { window: Window } }).jsdom?.window;
if (jsdomWindow?.localStorage != null) {
  Object.defineProperty(globalThis, "localStorage", {
    value: jsdomWindow.localStorage,
    writable: true,
    configurable: true,
  });
}
