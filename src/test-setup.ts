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

// jsdom 不实现 Element.scrollIntoView；组件 effect 会调用它，全局补桩防渲染期抛错。
// 需断言调用次数的测试须在 beforeEach 覆盖为 vi.fn()（本无操作桩非 vi.fn()，clearAllMocks 不会重置它）。
if (typeof Element.prototype.scrollIntoView !== "function") {
  Element.prototype.scrollIntoView = () => undefined;
}
