import "@fontsource-variable/oxanium/index.css";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { NexusProvider } from "./state";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <NexusProvider>
      <App />
    </NexusProvider>
  </React.StrictMode>,
);
