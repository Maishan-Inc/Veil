import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it("renders an accessible button with the requested variant and size", () => {
    render(
      <Button variant="primary" size="lg">
        Install engine
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Install engine" });
    expect(button).toBeInTheDocument();
    expect(button.getAttribute("data-variant")).toBe("primary");
    expect(button.getAttribute("data-size")).toBe("lg");
    expect(button.getAttribute("type")).toBe("button");
  });

  it("defaults to a secondary medium button", () => {
    render(<Button>Cancel</Button>);
    const button = screen.getByRole("button", { name: "Cancel" });
    expect(button.getAttribute("data-variant")).toBe("secondary");
    expect(button.getAttribute("data-size")).toBe("md");
  });
});
