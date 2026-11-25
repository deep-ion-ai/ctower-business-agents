import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TokensProjeto } from "./TokensProjeto";
import { apiPrototipoFalsa, tokensFicticios } from "./test/prototipoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PROJETO = "faturamento-exemplo";

describe("TokensProjeto: o cartão Identidade visual (#379)", () => {
  it("sem tokens, oferece extrair do código e avisa do wireframe neutro", async () => {
    const { api } = apiPrototipoFalsa();
    render(<TokensProjeto api={api} projeto={PROJETO} />);
    const cartao = screen.getByRole("region", { name: "Identidade visual" });
    expect(await within(cartao).findByText(/ainda não foram extraídos/)).toBeInTheDocument();
    expect(within(cartao).getByRole("note")).toHaveTextContent(/wireframe neutro/);
    fireEvent.click(within(cartao).getByRole("button", { name: "Extrair do código" }));
    await waitFor(() => expect(api.extrairTokens).toHaveBeenCalledWith(PROJETO));
    // Depois de extrair, a revisão abre com os papéis e o "a confirmar" em destaque.
    expect(await within(cartao).findByText("Rascunho, aguarda sua revisão")).toBeInTheDocument();
    expect(within(cartao).getByText("1 a confirmar")).toBeInTheDocument();
    const cores = within(cartao).getByRole("list", { name: "Papéis: Cores" });
    const erro = within(cores).getByText("--prototipo-cor-erro").closest("li")!;
    expect(erro).toHaveAttribute("data-situacao", "a-confirmar");
    expect(within(erro).getByText("a confirmar")).toBeInTheDocument();
    expect(within(erro).getByText(/darken/)).toBeInTheDocument();
    const primaria = within(cores).getByText("--prototipo-cor-primaria").closest("li")!;
    expect(primaria).toHaveTextContent("var(--fat-azul)");
    expect(primaria).toHaveTextContent("web/estilos/tema.css:3");
  });

  it("informa o valor de um papel a confirmar e volta um informado ao do código", async () => {
    const { api } = apiPrototipoFalsa(tokensFicticios());
    render(<TokensProjeto api={api} projeto={PROJETO} />);
    fireEvent.click(await screen.findByRole("button", { name: "Informar --prototipo-cor-erro" }));
    const campo = screen.getByRole("textbox", { name: "Valor de --prototipo-cor-erro" });
    fireEvent.change(campo, { target: { value: " #c62828 " } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(api.informarToken).toHaveBeenCalledWith(PROJETO, "cor-erro", "#c62828"),
    );
    await waitFor(() => expect(screen.queryByRole("textbox")).toBeNull());

    fireEvent.click(screen.getByRole("button", { name: "Voltar --prototipo-raio ao do código" }));
    await waitFor(() =>
      expect(api.informarToken).toHaveBeenLastCalledWith(PROJETO, "raio", undefined),
    );
  });

  it("aprova a identidade visual e tira o aviso do wireframe", async () => {
    const { api } = apiPrototipoFalsa(tokensFicticios());
    render(<TokensProjeto api={api} projeto={PROJETO} />);
    const cartao = screen.getByRole("region", { name: "Identidade visual" });
    const aprovar = await within(cartao).findByRole("button", {
      name: "Aprovar a identidade visual",
    });
    expect(within(cartao).getByRole("note")).toHaveTextContent(/wireframe neutro/);
    fireEvent.click(aprovar);
    await waitFor(() => expect(api.aprovarTokens).toHaveBeenCalledWith(PROJETO));
    expect(await within(cartao).findByText("Aprovada")).toBeInTheDocument();
    expect(within(cartao).getByRole("button", { name: "Identidade visual aprovada" })).toBeDisabled();
    expect(within(cartao).queryByRole("note")).toBeNull();
  });

  it("aprovada, a lista dos papéis começa fechada", async () => {
    const { api } = apiPrototipoFalsa(tokensFicticios("aprovado"));
    render(<TokensProjeto api={api} projeto={PROJETO} />);
    const revisar = await screen.findByRole("button", { name: "Revisar os papéis" });
    expect(screen.queryByRole("list", { name: "Papéis: Cores" })).toBeNull();
    fireEvent.click(revisar);
    expect(screen.getByRole("list", { name: "Papéis: Cores" })).toBeInTheDocument();
  });

  it("mostra a falha do comando", async () => {
    const { api } = apiPrototipoFalsa();
    api.extrairTokens.mockRejectedValueOnce("projeto sem código de interface");
    render(<TokensProjeto api={api} projeto={PROJETO} />);
    fireEvent.click(await screen.findByRole("button", { name: "Extrair do código" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("projeto sem código de interface");
  });
});
