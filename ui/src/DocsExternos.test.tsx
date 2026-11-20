import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SincronizacaoDocs } from "./DestinoDocumentacao";
import { TelaBase } from "./TelaBase";
import type { DocumentoAberto } from "./conhecimento";
import { PROJETO, apiConhecimentoFalsa, documentoFicticio } from "./test/conhecimentoFalso";
import { apiConfiguracoesFalsa, situacaoDocsFicticia } from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const EXTERNO = `${PROJETO}/externos/github-exemplo-faturamento-docs/processos/segunda-via.md`;

function externo(): DocumentoAberto {
  return {
    ...documentoFicticio(EXTERNO),
    caminho: EXTERNO,
    tipo: "doc-externo",
    titulo: "Segunda via do boleto",
    status: null,
    analise: null,
    somenteLeitura: true,
    externo: {
      repositorio: "GitHub exemplo/faturamento-docs",
      caminho: "docs/negocio/processos/segunda-via.md",
      commit: "c0ffee1234567",
      confianca: "externa",
    },
  };
}

describe("documentação externa (#277)", () => {
  it("abre o documento externo só para leitura, com a origem e o commit", async () => {
    const { api } = apiConhecimentoFalsa();
    api.abrirDocumento.mockImplementation(() => Promise.resolve(externo()));
    render(
      <TelaBase
        api={api}
        projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
        documentoInicial={EXTERNO}
        navegar={vi.fn()}
      />,
    );
    await screen.findByRole("article", { name: "Segunda via do boleto" });
    expect(screen.queryByRole("button", { name: "Editar" })).not.toBeInTheDocument();
    const nota = screen.getByRole("note");
    expect(nota).toHaveTextContent(/só leitura/);
    expect(nota).toHaveTextContent("docs/negocio/processos/segunda-via.md");
    expect(nota).toHaveTextContent("c0ffee1");
    expect(nota).toHaveTextContent("escrito fora do app");
  });

  it("mostra a situação, o aviso de desatualizada e sincroniza pelo botão", async () => {
    const api = apiConfiguracoesFalsa();
    render(<SincronizacaoDocs api={api} projeto={PROJETO} />);
    expect(
      await screen.findByText("o repositório de docs ainda não foi sincronizado"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Sincronizar agora" }));
    await waitFor(() => expect(api.sincronizarDocsExternos).toHaveBeenCalledWith(PROJETO));
    expect(await screen.findByText(/3 documentos externos/)).toBeInTheDocument();
    expect(screen.getByText("c0ffee1")).toBeInTheDocument();
    expect(
      screen.queryByText("o repositório de docs ainda não foi sincronizado"),
    ).not.toBeInTheDocument();
  });

  it("falha de rede aparece e não esconde o que já está na base", async () => {
    const api = apiConfiguracoesFalsa();
    api.sincronizarDocsExternos.mockRejectedValueOnce(
      new Error("não foi possível atualizar o clone: sem rede"),
    );
    api.situacaoDocsExternos.mockResolvedValue({
      ...situacaoDocsFicticia(),
      repositorio: "GitHub exemplo/faturamento-docs",
      commit: "beef0021234",
      sincronizadoMs: 1_700_000_000_000,
      importados: 1,
      erro: "sem rede",
      aviso: "a última sincronização falhou",
      pulados: [{ caminho: "docs/negocio/chave.md", motivo: "possível segredo" }],
    });
    render(<SincronizacaoDocs api={api} projeto={PROJETO} />);
    fireEvent.click(await screen.findByRole("button", { name: "Sincronizar agora" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("sem rede");
    expect(screen.getByText(/1 documento externo/)).toBeInTheDocument();
    expect(screen.getByText("1 arquivo pulado")).toBeInTheDocument();
  });
});
