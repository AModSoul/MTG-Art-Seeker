import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { createAppRouter } from "./router";

const router = createAppRouter();

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

// The prerendered HTML is replaced on the first render rather than hydrated: it is there for the
// first paint and for crawlers, and the release the download page shows can change between build
// and visit.
createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
