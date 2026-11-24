import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Cobertura, CoberturaArea } from "./cobertura";
import type { ApiConhecimento } from "./conhecimento";
import type { Tela } from "./Navegacao";
import { VisaoGeral } from "./VisaoGeral";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { PROJETO, apiConhecimentoFalsa, resumoFicticio } from "./test/conhecimentoFalso";
import { MAPA } from "./test/mapaFalso";
import type { AmbienteConferido } from "./atividades";
import type { ResumoProjeto } from "./resumoProjeto";
import type { MapaFuncionalidades } from "./funcionalidades";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("fora do Tauri"))),
  Channel: vi.fn(),
}));

function area(parcial: Partial<CoberturaArea> & Pick<CoberturaArea, "id" | "nome">): CoberturaArea {
  return {
    regras: 0,
    aRevalidar: 0,
    semConfirmacaoRecente: 0,
    termos: 0,
    documentos: 0,
    funcionalidades: 0,
    funcionalidadesSemDocumento: 0,
    arquivos: 0,
    centralidade: 0,
    idadeRecenteDias: null,
    idadeAntigaDias: null,
    lacuna: false,
    calor: 0,
    ...parcial,
  };
}

const COBERTURA: Cobertura = {
  areas: [
    area({ id: "contabilidade", nome: "Contabilidade", arquivos: 14, lacuna: true, calor: 0.4 }),
    area({ id: "multas", nome: "Multas por atraso", regras: 17 }),
    area({ id: "geral", nome: "Geral (cobrança)", regras: 7 }),
  ],
  pastas: [],
  sugestao: null,
  totais: { regras: 24, termos: 1, documentos: 0, aRevalidar: 0 },
};

const FLUXOS = ["perguntar", "explicar", "impacto", "reuniao", "revisar", "historias", "cenarios", "manual"] as const;
const porFluxo = (n: Partial<Record<(typeof FLUXOS)[number], number>>) =>
  FLUXOS.map((fluxo) => ({ fluxo, total: n[fluxo] ?? 0 }));

/** O resumo do fixture, com demandas e tipos de análise. */
const resumoBase = (): ResumoProjeto => {
  const r = resumoFicticio();
  return {
    ...r,
    analises: { ...r.analises, porFluxo: porFluxo({ perguntar: 1, impacto: 1 }) },
    demandas: { abertas: 1, encerradas: 1, semAnalise: 1 },
  };
};

/** Um projeto sem análise, demanda, regra nem sessão. */
const resumoVazio = (): ResumoProjeto => ({
  ...resumoFicticio({ atividades: [] }),
  analises: {
    total: 0,
    porEstado: { criada: 0, emAndamento: 0, concluida: 0, aprovada: 0 },
    porFluxo: porFluxo({}),
  },
  demandas: { abertas: 0, encerradas: 0, semAnalise: 0 },
  base: { regras: 0, aRevalidar: 0, termos: 0 },
  sessoes: { total: 0, ativas: 0, aguardandoAnalista: 0, concluidas: 0, encerradas: 0, porAnalise: [], vivas: [] },
  pedemAcao: 0,
});

function montar(
  resumo: ResumoProjeto | null = resumoBase(),
  extras: {
    mapa?: MapaFuncionalidades;
    cobertura?: Cobertura;
    ambiente?: AmbienteConferido;
    falha?: string;
  } = {},
) {
  const conhecimento = apiConhecimentoFalsa();
  const api: ApiConhecimento = {
    ...conhecimento.api,
    cobertura: vi.fn(() => Promise.resolve(extras.cobertura ?? COBERTURA)),
  };
  const navegar = vi.fn<(t: Tela) => void>();
  render(
    <VisaoGeral
      projeto={PROJETO}
      nomeProjeto="Faturamento Exemplo"
      resumo={resumo}
      falha={extras.falha ?? null}
      ambiente={"ambiente" in extras ? extras.ambiente : { pronto: true, pendencias: [] }}
      api={api}
      apiConfiguracoes={apiConfiguracoesFalsa()}
      apiMapa={{ mapa: () => Promise.resolve(extras.mapa ?? MAPA) }}
      navegar={navegar}
    />,
  );
  return { conhecimento, navegar };
}

const resumoCheio = (): ResumoProjeto =>
  resumoFicticio({
    demandas: { abertas: 1, encerradas: 1, semAnalise: 1 },
    analises: {
      total: 9,
      porEstado: { criada: 0, emAndamento: 2, concluida: 5, aprovada: 2 },
      porFluxo: [
        { fluxo: "perguntar", total: 2 },
        { fluxo: "explicar", total: 0 },
        { fluxo: "impacto", total: 1 },
        { fluxo: "reuniao", total: 0 },
        { fluxo: "revisar", total: 0 },
        { fluxo: "historias", total: 1 },
        { fluxo: "cenarios", total: 5 },
        { fluxo: "manual", total: 0 },
      ],
    },
    aguardandoRevisao: [
      {
        id: "A-0003",
        titulo: "Impacto do Open Finance",
        fluxo: "impacto",
        atualizadaMs: 1_700_000_000_000,
        afirmacoesDecididas: 2,
        afirmacoesTotal: 14,
        pendencias: 4,
      },
    ],
    atividades: [
      {
        tipo: "responder",
        ordem: 1,
        analise: "A-0008",
        demanda: null,
        titulo: "Multa por atraso",
        decididas: null,
        total: null,
        regras: null,
        atualizadaMs: 1_700_000_100_000,
      },
      {
        tipo: "revisar",
        ordem: 2,
        analise: "A-0003",
        demanda: null,
        titulo: "Impacto do Open Finance",
        decididas: 2,
        total: 14,
        regras: null,
        atualizadaMs: 1_700_000_000_000,
      },
      {
        tipo: "demanda-sem-analise",
        ordem: 3,
        analise: null,
        demanda: "D-0001",
        titulo: "Portabilidade: informar o saldo devedor",
        decididas: null,
        total: null,
        regras: null,
        atualizadaMs: null,
      },
      {
        tipo: "revalidar",
        ordem: 4,
        analise: null,
        demanda: null,
        titulo: null,
        decididas: null,
        total: null,
        regras: 2,
        atualizadaMs: null,
      },
    ],
  });

describe("VisaoGeral", () => {
  it("mostra o cabeçalho com origens e selos de saúde", async () => {
    montar();
    expect(await screen.findByRole("heading", { name: "Faturamento Exemplo" })).toBeInTheDocument();
    await screen.findByRole("region", { name: "Próximos passos" });
    expect(screen.getByText(/1 repositório de código|0 repositórios de código/)).toBeInTheDocument();
    const saude = await screen.findByRole("list", { name: "Saúde do projeto" });
    // O selo leva o símbolo e o rótulo em texto, não só a cor.
    expect(within(saude).getAllByRole("button").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Configurar projeto" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Nova análise" })).toBeInTheDocument();
  });

  it("mostra os três próximos passos e cada botão leva ao objeto", async () => {
    const { navegar } = montar(resumoCheio());
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    expect(within(passos).getByText("O agente aguarda você na A-0008")).toBeInTheDocument();
    expect(within(passos).getByText("Revisar e aprovar a A-0003")).toBeInTheDocument();
    expect(within(passos).getByText("Regras a revalidar")).toBeInTheDocument();
    // As demandas novas ficam fora até a #322, como nas Atividades (MOSTRAR_DEMANDAS_NOVAS).
    expect(within(passos).queryByText(/sem análise/)).not.toBeInTheDocument();
    expect(within(passos).getByRole("button", { name: "Ver as 3 atividades" })).toBeInTheDocument();

    fireEvent.click(within(passos).getByRole("button", { name: "Responder ao agente" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0008", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(within(passos).getByRole("button", { name: "Revisar a análise" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(within(passos).getByRole("button", { name: "Ver as regras" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base" });
    fireEvent.click(within(passos).getByRole("button", { name: "Ver as 3 atividades" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "inicio" });
  });

  it("os quatro números levam à lista correspondente", async () => {
    const { navegar } = montar(resumoCheio());
    const revisao = await screen.findByRole("button", { name: /^Aguardando sua revisão/ });
    expect(revisao).toHaveTextContent("1");
    fireEvent.click(revisao);
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "analise", projeto: PROJETO, situacao: "concluida" });
    const analises = screen.getByRole("button", { name: /^Análises/ });
    expect(analises).toHaveTextContent("9");
    expect(analises).toHaveTextContent("2 aprovadas · 2 em andamento");
    fireEvent.click(analises);
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "analise", projeto: PROJETO });
    fireEvent.click(screen.getByRole("button", { name: /^Demandas abertas/ }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demandas", projeto: PROJETO });
    fireEvent.click(screen.getByRole("button", { name: /^Regras na base/ }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base" });
  });

  it("mostra as análises por situação e por tipo, com rótulo e número em texto", async () => {
    montar(resumoCheio());
    const analises = await screen.findByRole("region", { name: "Análises" });
    const situacao = within(analises).getByRole("list", { name: "Análises por situação" });
    expect(situacao).toHaveTextContent("Aguardando revisão 5");
    expect(situacao).toHaveTextContent("Em andamento 2");
    expect(situacao).toHaveTextContent("Aprovadas 2");
    const tipos = within(analises).getByRole("list", { name: "Análises por tipo" });
    // Do mais para o menos usado; tipo sem análise não aparece.
    expect(within(tipos).getAllByRole("listitem").map((l) => l.textContent)).toEqual([
      "Cenários de teste5",
      "Perguntar ao sistema2",
      "Analisar impacto1",
      "Histórias e critérios1",
    ]);
  });

  it("lista as sessões vivas e a atividade recente com as datas que existem", async () => {
    const { navegar } = montar(resumoCheio());
    const painel = await screen.findByRole("region", { name: "Sessões e atividade" });
    const sessoes = within(painel).getByRole("list", { name: "Sessões ativas" });
    expect(sessoes).toHaveTextContent("A-0002");
    expect(sessoes).toHaveTextContent("aguarda você");
    expect(sessoes).toHaveTextContent("Parcelamento para pessoa jurídica");
    fireEvent.click(within(sessoes).getByRole("button", { name: "Parcelamento para pessoa jurídica" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0002", abertura: { tipo: "reabrir" } },
    });
    const recentes = within(painel).getByRole("list", { name: "Atividade recente" });
    expect(recentes).toHaveTextContent("A-0008: o agente aguarda você");
    expect(recentes).toHaveTextContent("A-0003 concluída pelo agente");
  });

  it("sessão sem análise ligada aparece com o cliente, sem link", async () => {
    const base = resumoCheio();
    montar({
      ...base,
      sessoes: {
        ...base.sessoes,
        vivas: [
          {
            id: "s9",
            estado: "ativa",
            cliente: "claude-code",
            analise: null,
            titulo: null,
            inicioMs: 0,
            atualizadaMs: 0,
            fimMs: null,
          },
        ],
      },
    });
    const painel = await screen.findByRole("region", { name: "Sessões e atividade" });
    const sessoes = within(painel).getByRole("list", { name: "Sessões ativas" });
    expect(sessoes).toHaveTextContent("Claude Code, ainda sem análise");
    expect(sessoes).toHaveTextContent("trabalhando");
    expect(within(sessoes).queryByRole("button")).toBeNull();
  });

  it("a atividade recente vem do histórico persistente, até 20 linhas", async () => {
    const linha = (i: number): ResumoProjeto["atividadeRecente"][number] => ({
      quandoMs: 1_700_000_000_000 - i,
      alvo: "A-0001",
      titulo: "Multa por atraso",
      mudanca: "afirmacao",
      estado: "em-andamento",
      quantas: 3,
    });
    montar({
      ...resumoCheio(),
      atividadeRecente: [
        { ...linha(0), mudanca: "estado", estado: "concluida", quantas: 1 },
        { ...linha(1), alvo: "R-0002", titulo: null, mudanca: "ata", quantas: 1 },
        { ...linha(2), alvo: "mapa", titulo: null, mudanca: "funcionalidade", quantas: 2 },
        ...Array.from({ length: 30 }, (_, i) => linha(i + 3)),
      ],
    });
    const painel = await screen.findByRole("region", { name: "Sessões e atividade" });
    const recentes = within(painel).getByRole("list", { name: "Atividade recente" });
    const itens = within(recentes).getAllByRole("listitem");
    expect(itens).toHaveLength(20);
    expect(itens[0]).toHaveTextContent("A-0001 · Multa por atraso: concluída pelo agente");
    expect(itens[1]).toHaveTextContent("Reunião R-0002: ata atualizada");
    expect(itens[2]).toHaveTextContent("Mapa de funcionalidades: funcionalidades atualizadas (2)");
    expect(itens[3]).toHaveTextContent("A-0001 · Multa por atraso: afirmações atualizadas (3)");
    // Com histórico, as datas derivadas do resumo não entram.
    expect(recentes).not.toHaveTextContent("A-0008: o agente aguarda você");
  });

  it("a cobertura mostra o nome da área e as lacunas em texto, e o estado do mapa", async () => {
    const { navegar } = montar(resumoCheio());
    const cobertura = await screen.findByRole("region", { name: "Cobertura da base" });
    const areas = await within(cobertura).findByRole("list", { name: "Cobertura por área" });
    expect(within(areas).getAllByRole("listitem").map((l) => l.textContent)).toEqual([
      "Multas por atraso17 regras",
      "Geral (cobrança)7 regras",
      "Contabilidadelacuna",
    ]);
    expect(areas).not.toHaveTextContent("/");
    // O mapa com funcionalidades mostra os números e leva a ele.
    fireEvent.click(await within(cobertura).findByRole("button", { name: "Abrir o mapa" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "mapa", projeto: PROJETO });
  });

  it("mapa vazio: ensina o que falta e oferece mapear", async () => {
    const { navegar } = montar(resumoCheio(), { mapa: { ...MAPA, total: 0, areas: [], documentadas: 0, pendentes: 0 } });
    const vazio = await screen.findByLabelText("Mapa de funcionalidades");
    expect(vazio).toHaveTextContent("Mapa de funcionalidades vazio");
    fireEvent.click(within(vazio).getByRole("button", { name: "Mapear com o Claude" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "mapa", projeto: PROJETO });
  });

  it("demandas: barra por situação e as abertas levam à demanda", async () => {
    const { navegar } = montar(resumoCheio());
    const painel = await screen.findByRole("region", { name: "Demandas" });
    const lista = await within(painel).findByRole("list", { name: "Demandas abertas" });
    fireEvent.click(within(lista).getByRole("button", { name: "Parcelamento PJ" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demanda", projeto: PROJETO, demanda: "D-0001" });
    expect(painel).toHaveTextContent("Abertas 1");
    expect(painel).toHaveTextContent("Encerradas 1");
    fireEvent.click(within(painel).getByRole("button", { name: "Abrir as demandas" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demandas", projeto: PROJETO });
  });

  it("projeto vazio: os blocos mostram estados vazios, não zeros soltos", async () => {
    const { navegar } = montar(resumoVazio(), { cobertura: { ...COBERTURA, areas: [] } });
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    expect(passos).toHaveTextContent("Nada espera por você");
    const numeros = screen.getAllByRole("button", { name: /^(Aguardando sua revisão|Análises|Demandas abertas|Regras na base)/ });
    expect(numeros).toHaveLength(4);
    for (const n of numeros) {
      expect(n).toHaveTextContent("Nenhuma");
      expect(n).not.toHaveTextContent(/\b0\b/);
    }
    expect(screen.getByRole("region", { name: "Análises" })).toHaveTextContent("Nenhuma análise ainda");
    expect(screen.getByRole("region", { name: "Sessões e atividade" })).toHaveTextContent("Nenhuma sessão ativa");
    expect(screen.getByRole("region", { name: "Sessões e atividade" })).toHaveTextContent("Ainda não aconteceu nada");
    expect(screen.getByRole("region", { name: "Demandas" })).toHaveTextContent("Nenhuma demanda ainda");
    await waitFor(() =>
      expect(screen.getByRole("region", { name: "Cobertura da base" })).toHaveTextContent("Nenhuma área na base ainda"),
    );
    fireEvent.click(within(screen.getByRole("region", { name: "Análises" })).getByRole("button", { name: "Nova análise" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "analise", projeto: PROJETO, nova: true });
  });

  it("o ambiente por preparar entra nos próximos passos", async () => {
    const { navegar } = montar(resumoVazio(), { ambiente: { pronto: false, pendencias: ["servidor MCP"] } });
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    fireEvent.click(within(passos).getByRole("button", { name: "Preparar" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "configurar", projeto: PROJETO });
  });

  it("projeto novo mostra os primeiros passos e como o app trabalha", async () => {
    const { navegar } = montar(resumoVazio(), { ambiente: { pronto: false, pendencias: ["servidor MCP"] } });
    const lista = await screen.findByRole("region", { name: "Primeiros passos" });
    expect(lista).toHaveTextContent("2 de 5");
    expect(screen.getByRole("region", { name: "Como o app trabalha" })).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Ligar o agente" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "configurar", projeto: PROJETO });
  });

  it("os primeiros passos somem quando a primeira análise é aprovada", async () => {
    const r = resumoVazio();
    r.analises = { ...r.analises, total: 1, porEstado: { ...r.analises.porEstado, aprovada: 1 } };
    montar(r);
    await screen.findByRole("region", { name: "Próximos passos" });
    expect(screen.queryByRole("region", { name: "Primeiros passos" })).not.toBeInTheDocument();
  });

  it("falha no resumo mostra o erro, sem os blocos", () => {
    montar(null, { falha: "disco cheio" });
    expect(screen.getByRole("alert")).toHaveTextContent("disco cheio");
    expect(screen.queryByRole("region", { name: "Próximos passos" })).not.toBeInTheDocument();
    // O cabeçalho continua: dá para configurar o projeto e criar uma análise.
    expect(screen.getByRole("button", { name: "Nova análise" })).toBeInTheDocument();
  });

  it("antes de ler o resumo, mostra o carregamento", () => {
    montar(null);
    expect(screen.getByText("Carregando…")).toBeInTheDocument();
  });
});
