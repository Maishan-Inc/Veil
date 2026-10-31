import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import "@testing-library/jest-dom/vitest";

// Vitest runs without `globals: true`, so Testing Library's automatic cleanup hook never
// registers. Unmount explicitly, or renders leak into the next test's queries.
afterEach(cleanup);
