import React from "react";
import ReactDOM from "react-dom/client";
import "./main.css";
import App from "./App";
import { ThemeProvider } from "./components/ui/Theme";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
