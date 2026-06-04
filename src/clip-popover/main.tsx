import React from "react";
import ReactDOM from "react-dom/client";
import { getResolved, subscribe } from "../theme/themeStore";
import { hideAndReturnFocus } from "../ipc/ipc-client";
import ClipPopoverApp from "./ClipPopoverApp";
import "./popover.css";

function applyTheme(): void {
  document.documentElement.dataset["theme"] = getResolved();
}

applyTheme();
subscribe(applyTheme);

document.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    // 走 hideAndReturnFocus 而非裸 hide：关闭面板的同时把焦点还给上一个外部 app（方案 C）。
    hideAndReturnFocus().catch((err: unknown) => {
      console.warn("[clip-popover] hideAndReturnFocus failed:", err);
    });
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ClipPopoverApp />
  </React.StrictMode>
);
