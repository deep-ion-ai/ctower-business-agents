import { act, createRef } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TerminalClaude, type ControleTerminal } from "./TerminalClaude";
import type { ApiTerminal, EventoTerminal, PedidoClaude, Tamanho } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
// O xterm.js precisa de canvas; os testes usam a tela falsa abaixo.
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

interface Abertura {
  sistema: string;
  pedido: PedidoClaude;
  tamanho: Tamanho;
  aoEvento: (e: EventoTerminal) => void;
  resolver: (id: number) => void;
  rejeitar: (e: unknown) => void;
}

function apiFalsa() {
  const aberturas: Abertura[] = [];
  const api = {
    abrir: vi.fn(
      (sistema: string, pedido: PedidoClaude, tamanho: Tamanho, aoEvento: Abertura["aoEvento"]) =>
        new Promise<number>((resolver, rejeitar) => {
          aberturas.push({ sistema, pedido, tamanho, aoEvento, resolver, rejeitar });
        }),
    ),
    escrever: vi.fn(() => Promise.resolve()),
    redimensionar: vi.fn(() => Promise.resolve()),
    enviarPrompt: vi.fn(() => Promise.resolve()),
    encerrar: vi.fn(() => Promise.resolve()),
  } satisfies ApiTerminal;
  return { api, aberturas };
}

function telaFalsa() {
  let digitar: (d: string) => void = () => undefined;
  const tamanho = { linhas: 24, colunas: 80 };
  const tela = {
    texto: "",
    tamanho,
    abrirEm: vi.fn(),
    escrever: vi.fn((t: string) => {
      tela.texto += t;
    }),
    aoDigitar: vi.fn((f: (d: string) => void) => {
      digitar = f;
    }),
    ajustar: vi.fn(() => ({ ...tamanho })),
    limpar: vi.fn(() => {
      tela.texto = "";
    }),
    focar: vi.fn(),
    descartar: vi.fn(),
    digitar: (d: string) => digitar(d),
  };
  return tela satisfies TelaTerminal & { digitar: (d: string) => void };
}

function montar(pedido?: PedidoClaude) {
  const { api, aberturas } = apiFalsa();
  const tela = telaFalsa();
  const ref = createRef<ControleTerminal>();
  const resultado = render(
    <TerminalClaude
      ref={ref}
      sistema="faturamento-exemplo"
      pedido={pedido}
      api={api}
      criarTela={() => tela}
    />,
  );
  return { api, aberturas, tela, ref, ...resultado };
}

async function abrirCom(aberturas: Abertura[], id: number, indice = 0) {
  await waitFor(() => expect(aberturas.length).toBeGreaterThan(indice));
  await act(async () => aberturas[indice].resolver(id));
}

describe("TerminalClaude", () => {
  it("abre o claude do sistema com o pedido e o tamanho da tela", async () => {
    const pedido: PedidoClaude = { promptInicial: "Inicie a análise A-0001 pelo ctower" };
    const { aberturas, tela, api } = montar(pedido);
    expect(screen.getByRole("status")).toHaveTextContent("Abrindo o Claude Code");
    await abrirCom(aberturas, 1);
    expect(aberturas[0].sistema).toBe("faturamento-exemplo");
    expect(aberturas[0].pedido).toEqual(pedido);
    expect(aberturas[0].tamanho).toEqual({ linhas: 24, colunas: 80 });
    expect(tela.abrirEm).toHaveBeenCalledWith(screen.getByTestId("terminal-tela"));
    expect(tela.focar).toHaveBeenCalled();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(api.redimensionar).toHaveBeenCalledWith(1, { linhas: 24, colunas: 80 });
  });

  it("mostra a saída e repassa o que o analista digita", async () => {
    const { aberturas, tela, api } = montar();
    await abrirCom(aberturas, 7);
    act(() => aberturas[0].aoEvento({ tipo: "saida", texto: "\x1b[1mClaude Code\x1b[0m" }));
    expect(tela.texto).toContain("Claude Code");
    tela.digitar("oi\r");
    expect(api.escrever).toHaveBeenCalledWith(7, "oi\r");
  });

  it("redimensiona com a janela", async () => {
    const { aberturas, tela, api } = montar();
    await abrirCom(aberturas, 3);
    tela.tamanho.linhas = 40;
    tela.tamanho.colunas = 120;
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    expect(api.redimensionar).toHaveBeenLastCalledWith(3, { linhas: 40, colunas: 120 });
  });

  it("envia prompts prontos e /desktop à sessão viva", async () => {
    const { aberturas, ref, api } = montar();
    await expect(ref.current!.enviarPrompt("antes")).rejects.toThrow();
    await abrirCom(aberturas, 5);
    await ref.current!.enviarPrompt("Aprofunde a afirmação 2");
    expect(api.enviarPrompt).toHaveBeenCalledWith(5, "Aprofunde a afirmação 2");
    await ref.current!.continuarNoDesktop();
    expect(api.enviarPrompt).toHaveBeenLastCalledWith(5, "/desktop");
  });

  it("encerra, mostra o código e reabre continuando a conversa", async () => {
    const { aberturas, ref, api, tela } = montar({
      promptInicial: "primeiro",
      argumentos: ["--x"],
    });
    await abrirCom(aberturas, 1);
    await act(() => ref.current!.encerrar());
    expect(api.encerrar).toHaveBeenCalledWith(1);
    act(() => aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 130 } }));
    expect(screen.getByRole("status")).toHaveTextContent("A sessão terminou (código 130).");
    // Depois do fim, o teclado não vai para lugar nenhum.
    tela.digitar("x");
    expect(api.escrever).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Continuar a conversa" }));
    await abrirCom(aberturas, 2, 1);
    expect(aberturas[1].pedido).toEqual({ argumentos: ["--x"], retomada: { modo: "continuar" } });
    expect(api.encerrar).toHaveBeenCalledTimes(1);
  });

  it("nova conversa reusa o pedido original", async () => {
    const pedido = { promptInicial: "primeiro" };
    const { aberturas } = montar(pedido);
    await abrirCom(aberturas, 1);
    act(() => aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 0 } }));
    fireEvent.click(screen.getByRole("button", { name: "Nova conversa" }));
    await abrirCom(aberturas, 2, 1);
    expect(aberturas[1].pedido).toEqual(pedido);
  });

  it("recusa com orientação quando o ambiente não está preparado", async () => {
    const { aberturas } = montar();
    await waitFor(() => expect(aberturas).toHaveLength(1));
    await act(async () =>
      aberturas[0].rejeitar({
        motivo: "ambienteNaoPreparado",
        mensagem: "o ambiente do Claude ainda não está preparado para este sistema",
        pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
      }),
    );
    const alerta = screen.getByRole("alert");
    expect(alerta).toHaveTextContent("ainda não está preparado");
    expect(alerta).toHaveTextContent("o servidor MCP do app não está registrado");
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    await waitFor(() => expect(aberturas).toHaveLength(2));
  });

  it("mostra o passo a passo quando o Claude Code não está instalado", async () => {
    const { aberturas } = montar();
    await waitFor(() => expect(aberturas).toHaveLength(1));
    await act(async () =>
      aberturas[0].rejeitar({
        motivo: "claudeNaoEncontrado",
        mensagem: "o Claude Code não foi encontrado neste computador",
        orientacao: [
          {
            texto: "Abra um terminal e rode o instalador oficial da Anthropic:",
            comando: "curl -fsSL https://claude.ai/install.sh | bash",
          },
        ],
      }),
    );
    expect(screen.getByText("curl -fsSL https://claude.ai/install.sh | bash")).toBeInTheDocument();
  });

  it("encerra o processo ao desmontar", async () => {
    const { aberturas, api, tela, unmount } = montar();
    await abrirCom(aberturas, 9);
    unmount();
    expect(api.encerrar).toHaveBeenCalledWith(9);
    expect(tela.descartar).toHaveBeenCalled();
  });

  it("encerra a sessão que termina de abrir depois de desmontado", async () => {
    const { aberturas, api, unmount } = montar();
    await waitFor(() => expect(aberturas).toHaveLength(1));
    unmount();
    expect(api.encerrar).not.toHaveBeenCalled();
    await act(async () => aberturas[0].resolver(4));
    expect(api.encerrar).toHaveBeenCalledWith(4);
  });
});
