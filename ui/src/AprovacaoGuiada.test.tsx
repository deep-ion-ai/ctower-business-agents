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
  it("com pendências: lista com Ir para e Revisar a(s) pendente(s), pela conta única", async () => {
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
    // A da análise como um todo (não concluída) aparece na lista, mas não conta (#222).
    fireEvent.click(screen.getByRole("button", { name: "Revisar a pendente" }));
    expect(aoRevisar).toHaveBeenCalledWith(COM_PENDENCIAS.pendencias);
    expect(screen.getByRole("button", { name: "Aprovar…" })).toBeDisabled();
  });

  it("escolhe o que vira regra: Resposta e Exemplo vêm desmarcados (#214)", async () => {
    const p: Prontidao = {
      ...PRONTA,
      areas: [
        { id: "cobranca", nome: "Cobrança" },
        { id: "prazos", nome: "Prazos" },
      ],
      areaObrigatoria: true,
      regras: [
        {
          afirmacao: 1,
          titulo: "Resposta",
          texto: "**Resposta.** Multa única.",
          secao: "resposta",
          secaoTitulo: "Resposta",
          sugerida: false,
        },
        {
          afirmacao: 2,
          titulo: "Carência de três dias",
          texto: "**Carência de três dias.** Sem multa.",
          secao: "regras_encontradas",
          secaoTitulo: "Regras encontradas",
          sugerida: true,
        },
        {
          afirmacao: 3,
          titulo: "Exemplo",
          texto: "**Exemplo.** R$ 100 com multa de R$ 2.",
          secao: "exemplo",
          secaoTitulo: "Exemplo",
          sugerida: false,
        },
        {
          afirmacao: 4,
          titulo: "Multa de 2%",
          texto: "A multa é de 2%.",
          secao: "regras_encontradas",
          secaoTitulo: "Regras encontradas",
          sugerida: true,
        },
      ],
    };
    const api = apiRevisao(p);
    api.aprovar = vi.fn(() =>
      Promise.resolve({
        jaAprovada: false,
        regras: [],
        termos: [],
        avisos: [],
      }),
    );
    render(<AprovacaoAnalise api={api} projeto="p" analise="A-0001" versao={1} />);

    const caixa = (n: number) =>
      screen.findByRole("checkbox", { name: `A afirmação ${n} vira regra` });
    expect(await caixa(1)).not.toBeChecked();
    expect(await caixa(2)).toBeChecked();
    expect(await caixa(3)).not.toBeChecked();
    expect(await caixa(4)).toBeChecked();
    expect(
      screen.getByText(/2 de 4 afirmações confirmadas vão virar regra/),
    ).toBeInTheDocument();
    // Só as marcadas mostram título e área; o título sugerido vem preenchido, sem markdown.
    expect(screen.queryByLabelText("Título da regra da afirmação 1")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Título da regra da afirmação 2")).toHaveValue(
      "Carência de três dias",
    );
    expect(screen.queryByLabelText(/Área da afirmação/)).not.toBeInTheDocument();

    // Desmarca a 4 e troca a área só da 2; a área do topo vale para as demais.
    fireEvent.click(await caixa(4));
    fireEvent.click(screen.getByRole("button", { name: "Trocar a área da afirmação 2" }));
    fireEvent.change(screen.getByLabelText("Área da afirmação 2"), {
      target: { value: "prazos" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Aprovar e gravar na base" }));
    expect(api.aprovar).toHaveBeenCalledWith("p", "A-0001", {
      area: null,
      afirmacoes: { 2: { area: "prazos", titulo: null } },
      regras: [2],
    });
  });

  it("nada marcado: a área não é pedida e nenhuma regra é enviada", async () => {
    const p: Prontidao = {
      ...PRONTA,
      areas: [
        { id: "cobranca", nome: "Cobrança" },
        { id: "prazos", nome: "Prazos" },
      ],
      areaObrigatoria: true,
      regras: [
        {
          afirmacao: 1,
          titulo: "Resposta",
          texto: "Sim.",
          secao: "resposta",
          secaoTitulo: "Resposta",
          sugerida: false,
        },
      ],
    };
    const api = apiRevisao(p);
    api.aprovar = vi.fn(() =>
      Promise.resolve({
        jaAprovada: false,
        regras: [],
        termos: [],
        avisos: [],
      }),
    );
    render(<AprovacaoAnalise api={api} projeto="p" analise="A-0001" versao={1} />);
    const aprovar = await screen.findByRole("button", {
      name: "Aprovar e gravar na base",
    });
    expect(aprovar).toBeEnabled();
    fireEvent.click(aprovar);
    expect(api.aprovar).toHaveBeenCalledWith("p", "A-0001", {
      area: null,
      afirmacoes: {},
      regras: [],
    });
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
      "Aguardando sua revisão · 1 pendente",
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
