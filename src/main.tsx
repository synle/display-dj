import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";

/** Disable the browser right-click context menu (Reload / Inspect Element) in production */
document.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
