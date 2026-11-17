import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaBase } from "./TelaBase";
import { PROJETO, REGRA, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar() {
  const falsa = apiConhecimentoFalsa();
  const aoPendencia = vi.fn();
  render(
    <TelaBase
      api={falsa.api}
      projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
      documentoInicial={REGRA}
      navegar={vi.fn()}
      aoPendencia={aoPendencia}
    />,
  );
  return { ...falsa, aoPendencia };
}

async function editar() {
  await screen.findByRole("article", { name: /Passada a carência/ });
  fireEvent.click(screen.getByRole("button", { name: "Editar" }));
  return screen.findByRole("textbox", { name: "Texto" });
}

describe("TelaBase: editar o documento (#220)", () => {
  it("abre com a lista de documentos, sem esperar a primeira busca", async () => {
    const { api } = montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    expect(api.buscar).toHaveBeenCalledWith("", { projeto: undefined, tipos: [], deslocamento: 0 });
    expect(within(resultados).getByText(/documentos na base/)).toBeInTheDocument();
  });

  it("edita, mostra a prévia, salva e a busca refeita enxerga o texto novo", async () => {
    const { api } = montar();
    const campo = await editar();
    expect((campo as HTMLTextAreaElement).value).toContain("# RN-COB-001");
    expect(screen.getByText("Sem alterações")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();

    fireEvent.change(campo, { target: { value: "# RN-COB-001\n\nO termo **zebrafish** vale." } });
    expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();
    const previa = screen.getByRole("region", { name: "Prévia" });
    expect(within(previa).getByText("zebrafish", { selector: "strong" })).toBeInTheDocument();

    const buscas = api.buscar.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(api.salvarDocumento).toHaveBeenCalledWith(
        REGRA,
        "# RN-COB-001\n\nO termo **zebrafish** vale.",
        "v1",
      ),
    );
    expect(await screen.findByText(/Salvo\./)).toBeInTheDocument();
    expect(screen.getByText("Sem alterações")).toBeInTheDocument();
    // Depois de salvar, a lista é refeita contra o índice.
    await waitFor(() => expect(api.buscar.mock.calls.length).toBeGreaterThan(buscas));

    // De volta ao documento, ele mostra o texto salvo.
    fireEvent.click(screen.getByRole("button", { name: "Voltar ao documento" }));
    const doc = await screen.findByRole("article", { name: /Passada a carência/ });
    expect(within(doc).getByText("zebrafish", { selector: "strong" })).toBeInTheDocument();
  });

  it("sair com alteração pendente mostra Alterações não salvas e avisa a navegação", async () => {
    const { aoPendencia } = montar();
    const campo = await editar();
    fireEvent.change(campo, { target: { value: "outro texto" } });
    await waitFor(() => expect(aoPendencia).toHaveBeenLastCalledWith(true));

    fireEvent.click(screen.getByRole("button", { name: "Voltar ao documento" }));
    const dialogo = await screen.findByRole("dialog", { name: "Alterações não salvas" });
    fireEvent.click(within(dialogo).getByRole("button", { name: "Continuar editando" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Texto" })).toHaveValue("outro texto");

    fireEvent.click(screen.getByRole("button", { name: "Voltar ao documento" }));
    const outro = await screen.findByRole("dialog", { name: "Alterações não salvas" });
    fireEvent.click(within(outro).getByRole("button", { name: "Descartar e voltar" }));
    expect(await screen.findByRole("article", { name: /Passada a carência/ })).toBeInTheDocument();
    await waitFor(() => expect(aoPendencia).toHaveBeenLastCalledWith(false));
  });

  it("descartar volta ao texto salvo", async () => {
    const { api } = montar();
    const campo = await editar();
    const original = (campo as HTMLTextAreaElement).value;
    fireEvent.change(campo, { target: { value: "rascunho" } });
    fireEvent.click(screen.getByRole("button", { name: "Descartar alterações" }));
    const dialogo = await screen.findByRole("dialog", { name: "Descartar as alterações" });
    fireEvent.click(within(dialogo).getByRole("button", { name: "Descartar" }));
    expect(screen.getByRole("textbox", { name: "Texto" })).toHaveValue(original);
    expect(api.salvarDocumento).not.toHaveBeenCalled();
  });

  it("arquivo alterado fora do app: oferece reabrir ou sobrescrever", async () => {
    const { api } = montar();
    const campo = await editar();
    api.salvarDocumento.mockRejectedValueOnce(new Error("conflito: mudou fora do app"));
    fireEvent.change(campo, { target: { value: "meu texto" } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    const dialogo = await screen.findByRole("dialog", { name: "O arquivo mudou fora do app" });
    fireEvent.click(within(dialogo).getByRole("button", { name: "Sobrescrever com o meu texto" }));
    await waitFor(() => expect(api.salvarDocumento).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/Salvo\./)).toBeInTheDocument();
  });
});
