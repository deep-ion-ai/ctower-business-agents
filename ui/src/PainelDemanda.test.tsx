import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AvisoTranscriptGuardado, PainelDemanda } from "./PainelDemanda";
import {
  ATA,
  DEMANDA,
  PROJETO,
  apiConhecimentoFalsa,
  demandasFicticias,
  painelFicticio,
} from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar(falsa = apiConhecimentoFalsa()) {
  const navegar = vi.fn();
  render(<PainelDemanda api={falsa.api} projeto={PROJETO} demanda="D-0001" navegar={navegar} />);
  return { ...falsa, navegar };
}

describe("PainelDemanda", () => {
  it("mostra o cabeçalho e a linha do tempo em ordem", async () => {
    const { api, navegar } = montar();
    expect(
      await screen.findByRole("heading", { name: "D-0001: Parcelamento PJ" }),
    ).toBeInTheDocument();
    expect(api.painelDemanda).toHaveBeenCalledWith(PROJETO, "D-0001");
    expect(screen.getByText("aberta")).toHaveAttribute("data-tone", "running");
    expect(screen.getByText(/Chave FAT-12/)).toBeInTheDocument();

    const linha = screen.getByRole("region", { name: "Linha do tempo" });
    const itens = within(linha).getAllByRole("listitem");
    expect(itens.map((i) => i.getAttribute("data-tipo"))).toEqual([
      "reuniao",
      "analise",
      "sessao",
      "reuniao",
    ]);
    expect(itens[0]).toHaveTextContent("Reunião R-0001: Levantar o pedido");
    expect(within(itens[0]).getByText("ata rascunho")).toBeInTheDocument();
    expect(within(itens[3]).getByText("ata aprovada")).toBeInTheDocument();
    expect(within(itens[2]).getByText(/Aguardando você/)).toBeInTheDocument();
    // Sem a guarda do transcript (#136), nenhum aviso.
    expect(screen.queryByText(/Transcript guardado/)).toBeNull();

    fireEvent.click(within(itens[1]).getByRole("button", { name: /Análise A-0003/ }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(within(itens[3]).getByRole("button", { name: "abrir a ata" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base", documento: ATA });
    fireEvent.click(screen.getByRole("button", { name: "abrir o demanda.md" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base", documento: DEMANDA });
  });

  it("mostra as pendências, as dúvidas e os próximos passos com a reunião de origem", async () => {
    montar();
    const pendencias = await screen.findByRole("region", { name: "Pendências" });
    expect(within(pendencias).getByText("P-1")).toBeInTheDocument();
    expect(pendencias).toHaveTextContent("Enviar a tabela de juros do parcelamento");
    expect(pendencias).toHaveTextContent("responsável: Ana (PO)");
    expect(pendencias).toHaveTextContent("prazo: 2026-11-20");
    expect(pendencias).toHaveTextContent("reunião R-0002 (2026-11-05)");
    const duvidas = screen.getByRole("region", { name: "Dúvidas em aberto" });
    expect(within(duvidas).getByText("DV-1")).toBeInTheDocument();
    expect(duvidas).toHaveTextContent("reunião R-0002");
    const passos = screen.getByRole("region", { name: "Próximos passos" });
    expect(passos).toHaveTextContent("Nenhum próximo passo nas atas aprovadas.");
  });

  it("cria a análise já ligada à demanda", async () => {
    const { api, navegar } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto" }));
    const form = screen.getByRole("form", { name: "Analisar impacto" });
    expect(form).toHaveTextContent("A análise fica ligada à demanda D-0001.");
    // Sem escolha de projeto, issue ou outra demanda.
    expect(within(form).queryByLabelText(/Issue do projeto/)).toBeNull();
    expect(within(form).queryByLabelText("Demanda (opcional)")).toBeNull();
    fireEvent.change(within(form).getByLabelText("Texto da demanda"), {
      target: { value: "Permitir parcelamento em até 3 vezes." },
    });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        PROJETO,
        "impacto",
        "Permitir parcelamento em até 3 vezes.",
        undefined,
        "D-0001",
      ),
    );
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: {
        id: "A-0003",
        abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0003 pelo ctower" },
      },
    });
  });

  it("encerra a demanda e, encerrada, não cria análise", async () => {
    const falsa = apiConhecimentoFalsa();
    const { api } = montar(falsa);
    fireEvent.click(await screen.findByRole("button", { name: "Encerrar a demanda" }));
    await waitFor(() => expect(api.encerrarDemanda).toHaveBeenCalledWith(PROJETO, "D-0001"));

    falsa.api.painelDemanda.mockResolvedValue(
      painelFicticio({ demanda: { ...demandasFicticias()[0], status: "encerrada" } }),
    );
    // Relê quando a base muda.
    falsa.avisar();
    expect(await screen.findByRole("button", { name: "Reabrir a demanda" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Analisar impacto" })).toBeNull();
    expect(screen.getByText(/Reabra a demanda para criar/)).toBeInTheDocument();
  });

  it("mostra a falha ao ler a demanda", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.painelDemanda.mockRejectedValueOnce("a demanda D-0001 não existe");
    montar(falsa);
    expect(await screen.findByRole("alert")).toHaveTextContent("a demanda D-0001 não existe");
  });
});

describe("AvisoTranscriptGuardado", () => {
  it("mostra a data em que o transcript é apagado e destaca quando vence em breve", () => {
    const { rerender } = render(
      <AvisoTranscriptGuardado guardado={{ apagaEm: "2026-12-10", venceEmBreve: false }} />,
    );
    expect(screen.getByRole("note")).toHaveTextContent(
      "Transcript guardado sem aprovação, apagado em 2026-12-10.",
    );
    rerender(<AvisoTranscriptGuardado guardado={{ apagaEm: "2026-11-18", venceEmBreve: true }} />);
    expect(screen.getByRole("alert")).toHaveAttribute("data-tone", "attention");
  });
});
