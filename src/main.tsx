import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import { Provider } from "@/components/ui/provider";
import { Toaster } from "@/components/ui/toaster";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

createRoot(root).render(
  <StrictMode>
    <Provider>
      <App />
      <Toaster />
    </Provider>
  </StrictMode>,
);
