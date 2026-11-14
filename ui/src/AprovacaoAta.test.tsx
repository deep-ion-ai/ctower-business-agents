import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AprovacaoAta } from "./AprovacaoAta";
import { PainelAnalise } from "./PainelAnalise";
import {
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";
import { apiReuniaoFalsa, prontidaoFicticia } from "./test/reunioesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const PROJETO = "faturamento-exemplo";

function montar(api = apiReuniaoFalsa()) {
  const aoAprovar = vi.fn();
  render(
    <AprovacaoAta api={api} projeto={PROJETO} analise="A-0005" versao={3} aoAprovar={aoAprovar} />,
  );
  return { api, aoAprovar };
}

describe("AprovacaoAta", () => {
  it("sem confirmar o segundo aviso, a ata não é aprovada", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Aprovar a ata" }));
    expect(api.prontidaoAta).toHaveBeenCalledWith(PROJETO, "A-0005");
    const dialogo = screen.getByRole("dialog", { name: "Aprovar a ata e apagar o transcript?" });
    expect(dialogo).toHaveTextContent("as citações da ata deixam de ser conferíveis");
    expect(dialogo).toHaveTextContent("3 citação(ões) deixam de ser conferíveis");
    // O botão seguro tem o foco.
    expect(within(dialogo).getByRole("button", { name: "Cancelar" })).toHaveFocus();

    fireEvent.click(within(dialogo).getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    // Esc também cancela.
    fireEvent.click(screen.getByRole("button", { name: "Aprovar a ata" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.aprovarAta).not.toHaveBeenCalled();
  });

  it("confirmando, aprova a ata e apaga o transcript", async () => {
    const { api, aoAprovar } = montar();
    expect(
      await screen.findByRole("list", { name: "Avisos da aprovação" }),
    ).toHaveTextContent("1 participante(s) sem papel confirmado");
    expect(screen.getByText(/Ao aprovar, o transcript é apagado/)).toBeInTheDocument();

    api.prontidaoAta.mockResolvedValue(
      prontidaoFicticia({
        status: "aprovada",
        estadoAnalise: "aprovada",
        concluida: true,
        podeAprovar: false,
        transcriptGuardado: false,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Aprovar a ata" }));
    fireEvent.click(screen.getByRole("button", { name: "Aprovar e apagar o transcript" }));
    await waitFor(() => expect(api.aprovarAta).toHaveBeenCalledWith(PROJETO, "A-0005", true));
    const resultado = await screen.findByRole("status", {
      name: "Resultado da aprovação da ata",
    });
    expect(resultado).toHaveTextContent("Ata aprovada em 2026-11-14. O transcript foi apagado.");
    expect(resultado).toHaveTextContent("3 citação(ões) marcadas como conferidas em 2026-11-14");
    expect(resultado).toHaveTextContent("já estão no painel da demanda");
    expect(aoAprovar).toHaveBeenCalled();
    // A prontidão é relida: a aprovação não é mais oferecida.
    await waitFor(() => expect(screen.queryByRole("button", { name: "Aprovar a ata" })).toBeNull());
  });

  it("mostra as pendências e não deixa aprovar", async () => {
    const api = apiReuniaoFalsa();
    api.prontidaoAta.mockResolvedValue(
      prontidaoFicticia({
        podeAprovar: false,
        pendencias: ["o agente ainda não concluiu a ata (concluir_analise)"],
      }),
    );
    montar(api);
    expect(await screen.findByRole("list", { name: "Pendências" })).toHaveTextContent(
      "concluir_analise",
    );
    expect(screen.getByRole("button", { name: "Aprovar a ata" })).toBeDisabled();
  });

  it("mostra a falha da aprovação e deixa tentar de novo", async () => {
    const api = apiReuniaoFalsa();
    api.aprovarAta.mockRejectedValueOnce(
      "o transcript da reunião R-0001 não foi apagado (disco cheio); a ata continua em rascunho",
    );
    const { aoAprovar } = montar(api);
    fireEvent.click(await screen.findByRole("button", { name: "Aprovar a ata" }));
    fireEvent.click(screen.getByRole("button", { name: "Aprovar e apagar o transcript" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a ata continua em rascunho");
    expect(aoAprovar).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole("button", { name: "Aprovar a ata" })).toBeEnabled());
  });

  it("ata aprovada sem a aprovação concluída avisa e deixa aprovar de novo", async () => {
    const api = apiReuniaoFalsa();
    api.prontidaoAta.mockResolvedValue(
      prontidaoFicticia({ status: "aprovada", transcriptGuardado: true }),
    );
    montar(api);
    expect(await screen.findByRole("alert")).toHaveTextContent("ficou incompleta");
    expect(screen.getByRole("button", { name: "Aprovar a ata" })).toBeEnabled();
  });

  it("aprovação concluída só informa", async () => {
    const api = apiReuniaoFalsa();
    api.prontidaoAta.mockResolvedValue(
      prontidaoFicticia({ status: "aprovada", concluida: true, podeAprovar: false }),
    );
    montar(api);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Ata aprovada: o transcript já foi apagado.",
    );
    expect(screen.queryByRole("button", { name: "Aprovar a ata" })).toBeNull();
  });
});

describe("PainelAnalise de uma reunião", () => {
  it("aprova pela ata, não pela aprovação das análises", async () => {
    const analise = apiAnaliseFalsa(
      vistaFicticia({}, { fluxo: "reuniao", reuniao: "R-0001", titulo: "Reunião R-0001" }),
    );
    const reuniao = apiReuniaoFalsa();
    const revisao = {
      revisar: vi.fn(),
      prontidao: vi.fn(),
      aprovar: vi.fn(),
      escolherDestino: vi.fn(),
      exportar: vi.fn(),
    };
    render(
      <PainelAnalise
        api={analise.api}
        projeto={PROJETO}
        analise="A-0001"
        abertura={{ tipo: "reabrir" }}
        aoVoltar={vi.fn()}
        apiTerminal={apiTerminalFalsa().api}
        apiConversa={apiConversaFalsa()}
        apiReuniao={reuniao}
        apiRevisao={revisao}
        criarTela={() => telaFalsa()}
      />,
    );
    // A aprovação fica no menu "Aprovar" da barra do painel.
    const barra = await screen.findByRole("banner");
    fireEvent.click(await within(barra).findByRole("button", { name: "Aprovar" }));
    expect(screen.getByRole("region", { name: "Aprovar a ata" })).toBeVisible();
    expect(await screen.findByRole("region", { name: "Aprovação da ata" })).toBeVisible();
    expect(reuniao.prontidaoAta).toHaveBeenCalledWith(PROJETO, "A-0001");
    expect(revisao.prontidao).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Aprovar análise" })).toBeNull();
  });
});
