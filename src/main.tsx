import React from "react";
import ReactDOM from "react-dom/client";
import "./main.css";
import App from "./App";
import { ThemeProvider } from "./components/ui/Theme";

// Transparent background for the undecorated floating window
document.documentElement.style.background = "transparent";
document.body.style.background = "transparent";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
