import type { Preview } from "@storybook/react-vite";

import { basePreview, resetUi } from "../src/storybook.tsx";

const preview: Preview = {
  ...basePreview,
  beforeEach: () => {
    resetUi();
  },
};

export default preview;
