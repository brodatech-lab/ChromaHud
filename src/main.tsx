import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Overlay from "./overlay/Overlay";
import Settings from "./settings/Settings";
import "./index.css";

// Both windows load the same bundle; the Tauri window label decides which view to render.
const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{label === "settings" ? <Settings /> : <Overlay />}</React.StrictMode>,
);
