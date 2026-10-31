import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Modal } from "./Modal";

describe("Modal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <Modal open={false} title="Hidden" onClose={() => undefined}>
        Body
      </Modal>,
    );
    expect(container.querySelector("[data-veil-modal]")).toBeNull();
  });

  it("renders the dialog with the title and body when open", () => {
    render(
      <Modal open title="Edit profile" onClose={() => undefined}>
        Hello body
      </Modal>,
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByText("Edit profile")).toBeInTheDocument();
    expect(screen.getByText("Hello body")).toBeInTheDocument();
  });

  it("closes when Escape is pressed", () => {
    const onClose = vi.fn();
    render(
      <Modal open title="X" onClose={onClose}>
        body
      </Modal>,
    );
    const backdrop = screen.getByRole("dialog").parentElement as HTMLElement;
    fireEvent.keyDown(backdrop, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes when the backdrop itself is clicked", () => {
    const onClose = vi.fn();
    render(
      <Modal open title="X" onClose={onClose}>
        body
      </Modal>,
    );
    const backdrop = screen.getByRole("dialog").parentElement as HTMLElement;
    fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does NOT close when the panel itself is clicked", () => {
    const onClose = vi.fn();
    render(
      <Modal open title="X" onClose={onClose}>
        <button>Inside</button>
      </Modal>,
    );
    fireEvent.click(screen.getByText("Inside"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("renders a footer slot when provided", () => {
    render(
      <Modal open title="X" onClose={() => undefined} footer={<span>Footer goes here</span>}>
        body
      </Modal>,
    );
    expect(screen.getByText("Footer goes here")).toBeInTheDocument();
  });
});
