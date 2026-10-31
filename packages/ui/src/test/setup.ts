import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// React Testing Library's auto-cleanup only happens when the test framework
// integration is enabled. Vitest in CJS-less mode picks it up automatically
// in some versions; do it explicitly to be safe — Modal renders through a
// portal into document.body and would otherwise persist across tests.
afterEach(() => {
  cleanup();
});
