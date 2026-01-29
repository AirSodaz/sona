/**
 * Application entry point.
 * Renders the root React component into the DOM.
 */

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import './i18n';

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
