import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaProjeto } from "./TelaProjeto";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar(falsa = apiConhecimentoFalsa()) {
  const navegar = vi.fn();
  render(
    <TelaProjeto
      api={falsa.api}
      projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
      projetoInicial={PROJETO}
      navegar={navegar}
    />,
  );
  return { ...falsa, navegar };
}

describe("TelaProjeto", () => {
  it("não mostra mais as regras, o glossário, as áreas nem a conferência (#328)", async () => {
    const { api } = montar();
    await screen.findByRole("region", { name: "Mapa do projeto" });
    expect(screen.queryByRole("region", { name: "Regras" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Glossário" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Áreas" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Conferir de novo" })).not.toBeInTheDocument();
    expect(api.revalidar).not.toHaveBeenCalled();
  });

  it("navega pelo mapa e mapeia quando ainda não há mapa", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.mapa.mockResolvedValueOnce(null);
    montar(falsa);
    const mapa = await screen.findByRole("region", { name: "Mapa do projeto" });
    expect(await within(mapa).findByText(/ainda não foi gerado/)).toBeInTheDocument();
    fireEvent.click(within(mapa).getByRole("button", { name: "Mapear agora" }));
    expect(await within(mapa).findByText(/14 arquivos: SQL 3/)).toBeInTheDocument();
    expect(falsa.api.mapear).toHaveBeenCalledWith(PROJETO);
    expect(within(mapa).getByText(/backend → db/)).toBeInTheDocument();

    fireEvent.click(within(mapa).getByRole("button", { name: "db/" }));
    expect(await within(mapa).findByText("03_pkg_cobranca.sql")).toBeInTheDocument();
    expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, "db");
    fireEvent.click(within(mapa).getByRole("button", { name: "(raiz)" }));
    await waitFor(() => expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, ""));
  });

  it("o mapa usa termos de negócio, não os tipos técnicos (#227)", async () => {
    const falsa = apiConhecimentoFalsa();
    const raiz = (await falsa.api.mapa(PROJETO, ""))!;
    falsa.api.mapa.mockResolvedValue({
      ...raiz,
      modulo: {
        ...raiz.modulo,
        simbolos: [
          { chave: "campo", rotulo: "campo", quantidade: 30 },
          { chave: "metodo", rotulo: "metodo", quantidade: 20 },
          { chave: "construtor", rotulo: "construtor", quantidade: 5 },
          { chave: "tabela", rotulo: "tabela", quantidade: 6 },
        ],
      },
    });
    montar(falsa);
    const mapa = await screen.findByRole("region", { name: "Mapa do projeto" });
    expect(
      await within(mapa).findByText(
        "30 dados (atributos e valores fixos) · 25 operações (rotinas e funções) · 6 tabelas",
      ),
    ).toBeInTheDocument();
    expect(mapa).not.toHaveTextContent(/construtor|metodo|campo/);
  });

  it("pula as cadeias de pasta única ao entrar numa subpasta (#227)", async () => {
    const falsa = apiConhecimentoFalsa();
    const raiz = (await falsa.api.mapa(PROJETO, ""))!;
    const pasta = (caminho: string, subpastas: string[], diretos: string[] = []) => ({
      ...raiz,
      modulo: {
        ...raiz.modulo,
        caminho,
        subpastas: subpastas.map((c) => ({ caminho: c, arquivos: 4 })),
        arquivosDiretos: diretos,
        dependencias: [],
      },
    });
    const mapas: Record<string, ReturnType<typeof pasta>> = {
      "": pasta("", ["src"]),
      src: pasta("src", ["src/main"]),
      "src/main": pasta("src/main", ["src/main/java"]),
      "src/main/java": pasta("src/main/java", ["src/main/java/services", "src/main/java/telas"]),
      "src/main/java/services": pasta("src/main/java/services", [], ["Fatura.java"]),
    };
    falsa.api.mapa.mockImplementation((_s: string, m?: string) =>
      Promise.resolve(mapas[m ?? ""] ?? null),
    );
    montar(falsa);
    const mapa = await screen.findByRole("region", { name: "Mapa do projeto" });
    // Um clique em `src/` já leva até a primeira pasta com mais de um caminho.
    fireEvent.click(await within(mapa).findByRole("button", { name: "src/" }));
    expect(await within(mapa).findByRole("button", { name: "services/" })).toBeInTheDocument();
    expect(within(mapa).getByRole("button", { name: "telas/" })).toBeInTheDocument();
    expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, "src/main/java");
    // A trilha mostra o caminho inteiro e cada parte continua clicável.
    const trilha = within(within(mapa).getByRole("navigation", { name: "Pasta do mapa" }));
    expect(trilha.getAllByRole("button").map((b) => b.textContent)).toEqual([
      "(raiz)",
      "src",
      "main",
      "java",
    ]);
    fireEvent.click(trilha.getByRole("button", { name: "src" }));
    await waitFor(() => expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, "src"));
    expect(await within(mapa).findByRole("button", { name: "main/" })).toBeInTheDocument();
  });
});
