// The app as `src/main.tsx` starts it, with the recording's input already entered.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import { useInput } from "@/store";
import { system } from "@/theme";
import { Provider } from "@mtg-art/ui/provider";
import { Toaster } from "@mtg-art/ui/toaster";

const { input } = window.screenshotRecording;
if (input.kind === "list") {
  useInput.setState({ tab: "list", text: input.text });
} else {
  const { fileName, name, code, setName } = input.deck;
  useInput.setState({
    tab: "deck",
    deckQuery: input.query,
    deck: { fileName, name, code, setName },
    deckFolder: input.folder,
  });
}

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

createRoot(root).render(
  <StrictMode>
    <Provider system={system}>
      <App />
      <Toaster />
    </Provider>
  </StrictMode>,
);
