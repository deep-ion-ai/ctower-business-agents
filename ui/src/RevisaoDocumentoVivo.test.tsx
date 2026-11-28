import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EVENTO_DOCUMENTACAO } from "./documentacaoMapa";
import { chaveDoObjeto, secaoDaTela } from "./Navegacao";
import { RevisaoDocumentoVivo } from "./RevisaoDocumentoVivo";
import { apiDocumentacaoFalsa, COLUNAS, linha, sem, tabelaDe } from "./test/documentacaoFalsa";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const CAMINHO = "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md";

/** Dois rascunhos: o catálogo de F-FAT-001 (o aberto) e o caso de uso de F-FAT-002. */
function tabelaComFila() {
  return tabelaDe([
    linha("F-FAT-001", "Cancelar fatura", [
      sem("caso-de-uso"),
      {
        tipo: "catalogo-regras",
        caminho: CAMINHO,
        estado: { estado: "proposta_pendente", versao: "1.0", revisada: false },
        acao: "Revisar proposta",
      },
      ...COLUNAS.slice(2).map((c) => sem(c.tipo)),
    ]),
    linha("F-FAT-002", "Calcular multa", [
      {
        tipo: "caso-de-uso",
        caminho: "funcionalidades/cobranca/calcular-multa/caso-de-uso.md",
        estado: { estado: "proposta_pendente", versao: "1.0", revisada: true },
        acao: "Revisar proposta",
      },
      ...COLUNAS.slice(1).map((c) => sem(c.tipo)),
    ]),
  ]);
}

function abrir() {
  const { api } = apiDocumentacaoFalsa(tabelaComFila());
  const aoAbrir = vi.fn();
  const mudou = vi.fn();
  window.addEventListener(EVENTO_DOCUMENTACAO, mudou);
  render(
    <RevisaoDocumentoVivo
      projeto="faturamento-exemplo"
      documento={CAMINHO}
      api={api}
      aoAbrir={aoAbrir}
    />,
  );
  return { api, aoAbrir, mudou };
}

describe("RevisaoDocumentoVivo", () => {
  it("mostra o rascunho, os passos e a fila; publicar só depois de marcar como revisado", async () => {
    const { api, mudou } = abrir();
    const principal = await screen.findByRole("region", { name: "Revisão final" });
    expect(api.proposta).toHaveBeenCalledWith("faturamento-exemplo", CAMINHO);
    expect(principal).toHaveTextContent("Catálogo de regras de negócio · Cobrança");
    expect(within(principal).getAllByRole("heading", { level: 1 })[0]).toHaveTextContent(
      "Cancelar fatura",
    );
    expect(principal).toHaveTextContent("rascunho v1.0 · versão publicada: nenhuma");
    const passos = within(principal).getByRole("list", { name: "Passos da revisão final" });
    expect(within(passos).getAllByRole("listitem").map((p) => p.dataset.feito)).toEqual([
      "sim",
      "nao",
      "nao",
    ]);
    expect(within(principal).getByLabelText("Texto da proposta")).toHaveTextContent(
      "Uma fatura paga não pode ser cancelada.",
    );

    const fila = await screen.findByRole("complementary", { name: "A revisar" });
    expect(fila).toHaveTextContent("A revisar 2");
    expect(within(fila).getByRole("region", { name: "Especificação de caso de uso" })).toHaveTextContent(
      "Calcular multaF-FAT-002 · rascunho v1.0 · revisado",
    );

    const publicar = within(principal).getByRole("button", { name: "Publicar v1.0" });
    expect(publicar).toBeDisabled();
    expect(principal).toHaveTextContent("Publicar só depois de marcar como revisado");
    fireEvent.click(within(principal).getByRole("button", { name: "Marcar como revisado" }));
    await waitFor(() => expect(publicar).toBeEnabled());
    expect(within(passos).getAllByRole("listitem")[1].dataset.feito).toBe("sim");
    expect(mudou).toHaveBeenCalledTimes(1);

    fireEvent.click(publicar);
    await waitFor(() => expect(api.publicar).toHaveBeenCalledWith("faturamento-exemplo", CAMINHO));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Catálogo de regras de negócio v1.0 publicado.",
    );
    expect(within(passos).getAllByRole("listitem")[2].dataset.feito).toBe("sim");
    expect(mudou).toHaveBeenCalledTimes(2);
  });

  it("publicar e abrir o próximo segue a fila sem voltar à tabela", async () => {
    const { api, aoAbrir } = abrir();
    const principal = await screen.findByRole("region", { name: "Revisão final" });
    await screen.findByRole("complementary", { name: "A revisar" });
    fireEvent.click(within(principal).getByRole("button", { name: "Marcar como revisado" }));
    const seguir = within(principal).getByRole("button", { name: "Publicar e abrir o próximo" });
    await waitFor(() => expect(seguir).toBeEnabled());
    fireEvent.click(seguir);
    await waitFor(() => expect(api.publicar).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(aoAbrir).toHaveBeenCalledWith({
        caminho: "funcionalidades/cobranca/calcular-multa/caso-de-uso.md",
        funcionalidade: "F-FAT-002",
        titulo: "Revisão · F-FAT-002 · Caso de uso",
      }),
    );
  });

  it("um item da fila abre a revisão dele; o aberto fica marcado", async () => {
    const { aoAbrir } = abrir();
    const fila = await screen.findByRole("complementary", { name: "A revisar" });
    await waitFor(() => expect(within(fila).getAllByRole("listitem")).toHaveLength(2));
    // A fila segue a ordem das colunas: o caso de uso de F-FAT-002 vem antes do catálogo.
    const [outro, atual] = within(fila).getAllByRole("listitem");
    expect(atual).toHaveAttribute("aria-current", "page");
    expect(within(atual).getByRole("button")).toBeDisabled();
    fireEvent.click(within(outro).getByRole("button"));
    expect(aoAbrir).toHaveBeenCalledWith(
      expect.objectContaining({ funcionalidade: "F-FAT-002" }),
    );
  });

  it("descartar o rascunho encerra a revisão nesta aba", async () => {
    const { api } = abrir();
    const principal = await screen.findByRole("region", { name: "Revisão final" });
    fireEvent.click(within(principal).getByRole("button", { name: "Descartar rascunho" }));
    await waitFor(() => expect(api.descartar).toHaveBeenCalledWith("faturamento-exemplo", CAMINHO));
    expect(await screen.findByRole("status")).toHaveTextContent("Rascunho descartado.");
    expect(within(principal).queryByRole("button", { name: "Publicar v1.0" })).toBeNull();
  });

  it("sem o rascunho, diz que não foi possível abrir", async () => {
    const { api } = apiDocumentacaoFalsa(tabelaComFila());
    api.proposta.mockRejectedValueOnce("documento sem proposta");
    render(<RevisaoDocumentoVivo projeto="faturamento-exemplo" documento={CAMINHO} api={api} />);
    expect(await screen.findByText("Não foi possível abrir o rascunho.")).toBeInTheDocument();
  });

  it("a aba é uma por documento e fica na seção Funcionalidades", () => {
    const tela = (documento: string) =>
      ({ tipo: "revisao", projeto: "p", documento, titulo: "Revisão · F-FAT-001 · Caso de uso" }) as const;
    expect(chaveDoObjeto(tela("a.md"))).toBe(chaveDoObjeto(tela("a.md")));
    expect(chaveDoObjeto(tela("a.md"))).not.toBe(chaveDoObjeto(tela("b.md")));
    expect(secaoDaTela(tela("a.md"))).toBe("funcionalidades");
  });
});
