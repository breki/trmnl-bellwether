import { render, screen } from "@testing-library/svelte";
import { afterEach, describe, it, expect, vi } from "vitest";
import App from "./App.svelte";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("App", () => {
  it("renders the heading", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.reject(new Error())),
    );
    render(App);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
      "rustbase",
    );
  });
});
