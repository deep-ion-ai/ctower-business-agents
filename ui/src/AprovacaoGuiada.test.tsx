import { fireEvent, render, screen, within } from "@testing-library/react";
import { AprovacaoAnalise } from "./AprovacaoAnalise";
import { AprovacaoDialogo } from "./AprovacaoDialogo";
import { FaixaCodigoAlterado } from "./FaixaCodigoAlterado";
import { PainelAnalise } from "./PainelAnalise";
import type { Afirmacao } from "./analise";
import type { ApiRevisao, Prontidao } from "./revisao";
import {
  AFIRMACAO_MULTA,
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const PRONTA: Prontidao = {
  estado: "concluida",
  podeAprovar: true,
  pendencias: [],
  avisos: [],
  areas: [{ id: "cobranca", nome: "Cobrança" }],
  areaObrigatoria: false,
  regras: [{ afirmacao: 1, titulo: "Multa", texto: "x" }],
  incorretas: [],
  termosAceitos: [],
};

const COM_PENDENCIAS: Prontidao = {
  ...PRONTA,
  podeAprovar: false,
  pendencias: [
    { tipo: "afirmacao", numero: 2, motivo: "afirmação 2: sem decisão" },
    { tipo: "analise", numero: null, motivo: "a análise não foi concluída" },
  ],
};

function apiRevisao(p: Prontidao): ApiRevisao {
  return {
    revisar: vi.fn(),
    prontidao: vi.fn(() => Promise.resolve(p)),
    aprovar: vi.fn(),
    escolherDestino: vi.fn(),
    exportar: vi.fn(),
  };
}

const REVALIDAR: Afirmacao = {
  ...AFIRMACAO_MULTA,
  estado: "a-revalidar",
  evidencias: [{ ...AFIRMACAO_MULTA.evidencias[0], estado: "a-revalidar" }],
};

describe("Aprovação guiada", () => {
  it("com pendências: lista com Ir para e Revisar as N pendentes", async () => {
    const aoIrPara = vi.fn();
    const aoRevisar = vi.fn();
    render(
      <AprovacaoAnalise
        api={apiRevisao(COM_PENDENCIAS)}
        projeto="p"
        analise="A-0001"
        versao={1}
        aoIrPara={aoIrPara}
        aoRevisarPendentes={aoRevisar}
      />,
    );
    const lista = await screen.findByRole("list", { name: "Pendências" });
    // Só a pendência de afirmação tem destino no documento.
    expect(within(lista).getAllByRole("button", { name: /Ir para/ })).toHaveLength(1);
    fireEvent.click(within(lista).getByRole("button", { name: /Ir para/ }));
    expect(aoIrPara).toHaveBeenCalledWith(COM_PENDENCIAS.pendencias[0]);
    fireEvent.click(screen.getByRole("button", { name: "Revisar as 2 pendentes" }));
    expect(aoRevisar).toHaveBeenCalledWith(COM_PENDENCIAS.pendencias);
    expect(screen.getByRole("button", { name: "Aprovar…" })).toBeDisabled();
  });

  it("sem pendências: Aprovar e gravar na base", async () => {
    render(
      <AprovacaoAnalise api={apiRevisao(PRONTA)} projeto="p" analise="A-0001" versao={1} />,
    );
    expect(await screen.findByRole("button", { name: "Aprovar e gravar na base" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: /Revisar as/ })).not.toBeInTheDocument();
  });

  it("o diálogo prende o foco, fecha com Esc e devolve o foco", () => {
    const aoFechar = vi.fn();
    render(
      <>
        <button>abrir</button>
        <AprovacaoDialogo titulo="Aprovar" aoFechar={aoFechar}>
          <button>um</button>
        </AprovacaoDialogo>
      </>,
    );
    const dialogo = screen.getByRole("dialog", { name: "Aprovar" });
    const um = screen.getByRole("button", { name: "um" });
    const fechar = screen.getByRole("button", { name: "Fechar" });
    fechar.focus();
    fireEvent.keyDown(dialogo, { key: "Tab" });
    expect(um).toHaveFocus();
    fireEvent.keyDown(dialogo, { key: "Tab", shiftKey: true });
    expect(fechar).toHaveFocus();
    fireEvent.keyDown(dialogo, { key: "Escape" });
    expect(aoFechar).toHaveBeenCalled();
  });

  it("a faixa de código alterado lista arquivo e afirmações, pede revalidação e dispensa", () => {
    const aoVer = vi.fn();
    const aoPedir = vi.fn(() => true);
    render(
      <FaixaCodigoAlterado
        afirmacoes={[REVALIDAR, { ...AFIRMACAO_MULTA, numero: 2 }]}
        aoVer={aoVer}
        aoPedir={aoPedir}
      />,
    );
    const faixa = screen.getByRole("region", { name: "Código alterado" });
    expect(within(faixa).getByText("db/03_pkg_cobranca.sql")).toBeInTheDocument();
    expect(within(faixa).getByText(/afirmação 1/)).toBeInTheDocument();
    expect(within(faixa).queryByText(/afirmação 2/)).not.toBeInTheDocument();
    fireEvent.click(within(faixa).getByRole("button", { name: /Ver a mudança/ }));
    expect(aoVer).toHaveBeenCalledWith(REVALIDAR.evidencias[0], "afirmacao-1");
    fireEvent.click(within(faixa).getByRole("button", { name: "Pedir revalidação ao agente" }));
    expect(aoPedir).toHaveBeenCalled();
    expect(within(faixa).getByRole("status")).toHaveTextContent("Pedido de revalidação enviado");
    fireEvent.click(within(faixa).getByRole("button", { name: "Dispensar" }));
    expect(screen.queryByRole("region", { name: "Código alterado" })).not.toBeInTheDocument();
  });

  it("sem código alterado, a faixa não aparece", () => {
    render(
      <FaixaCodigoAlterado afirmacoes={[AFIRMACAO_MULTA]} aoVer={vi.fn()} aoPedir={vi.fn()} />,
    );
    expect(screen.queryByRole("region", { name: "Código alterado" })).not.toBeInTheDocument();
  });

  it("a barra: botão e selo acompanham o estado, e Ir para foca o cartão", async () => {
    const analise = apiAnaliseFalsa(
      vistaFicticia({}, { afirmacoes: [{ ...AFIRMACAO_MULTA, numero: 2 }] }),
    );
    render(
      <PainelAnalise
        api={analise.api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "reabrir" }}
        aoVoltar={vi.fn()}
        apiTerminal={apiTerminalFalsa().api}
        apiConversa={apiConversaFalsa()}
        apiRevisao={apiRevisao(COM_PENDENCIAS)}
        criarTela={() => telaFalsa()}
      />,
    );
    const barra = await screen.findByRole("banner");
    expect(await within(barra).findByRole("status", { name: "Situação da aprovação" })).toHaveTextContent(
      "Aguardando sua revisão · 2 pendentes",
    );
    fireEvent.click(within(barra).getByRole("button", { name: "Aprovar…" }));
    const dialogo = screen.getByRole("dialog", { name: "Aprovar a análise" });
    fireEvent.click(await within(dialogo).findByRole("button", { name: /Ir para/ }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await vi.waitFor(() => expect(screen.getByTestId("afirmacao-2")).toHaveFocus());
  });

  it("pronta para aprovar", async () => {
    const analise = apiAnaliseFalsa();
    render(
      <PainelAnalise
        api={analise.api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "reabrir" }}
        aoVoltar={vi.fn()}
        apiTerminal={apiTerminalFalsa().api}
        apiConversa={apiConversaFalsa()}
        apiRevisao={apiRevisao(PRONTA)}
        criarTela={() => telaFalsa()}
      />,
    );
    const barra = await screen.findByRole("banner");
    expect(await within(barra).findByRole("button", { name: "Aprovar análise" })).toBeEnabled();
    expect(within(barra).getByRole("status", { name: "Situação da aprovação" })).toHaveTextContent(
      "Pronta para aprovar",
    );
  });
});
