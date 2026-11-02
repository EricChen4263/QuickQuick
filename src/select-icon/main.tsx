import React from "react";
import ReactDOM from "react-dom/client";
import { getResolved, subscribe } from "../theme/themeStore";
import SelectIconApp from "./SelectIconApp";
import "./select-icon.css";

function applyTheme(): void {
  document.documentElement.dataset["theme"] = getResolved();
}

applyTheme();
subscribe(applyTheme);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SelectIconApp />
  </React.StrictMode>
);
