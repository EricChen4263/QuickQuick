import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getResolved, subscribe } from "../theme/themeStore";
import TransPopoverApp from "./TransPopoverApp";
import "./trans-popover.css";

function applyTheme(): void {
  document.documentElement.dataset["theme"] = getResolved();
}

applyTheme();
subscribe(applyTheme);

document.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    getCurrentWindow()
      .hide()
      .catch((err: unknown) => {
        console.warn("[trans-popover] hide failed:", err);
      });
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TransPopoverApp />
  </React.StrictMode>
);
