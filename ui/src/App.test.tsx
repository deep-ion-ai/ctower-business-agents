import { render, screen } from "@testing-library/react";
import { App, PRODUTO } from "./App";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("1.2.3")),
}));

describe("App", () => {
  it("mostra o nome do produto e a versão do app", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: PRODUTO })).toBeInTheDocument();
    expect(await screen.findByText("Versão 1.2.3")).toBeInTheDocument();
  });
});
