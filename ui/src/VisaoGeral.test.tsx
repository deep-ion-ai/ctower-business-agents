import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Cobertura, CoberturaArea } from "./cobertura";
import type { ApiConhecimento } from "./conhecimento";
import type { Tela } from "./Navegacao";
import { VisaoGeral } from "./VisaoGeral";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import { MAPA } from "./test/mapaFalso";
import { apiResumoFalsa, resumoFicticio, resumoVazio } from "./test/resumoFalso";
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

function montar(
  resumo: ResumoProjeto = resumoFicticio(),
  extras: { mapa?: MapaFuncionalidades; cobertura?: Cobertura } = {},
) {
  const conhecimento = apiConhecimentoFalsa();
  const api: ApiConhecimento = {
    ...conhecimento.api,
    cobertura: vi.fn(() => Promise.resolve(extras.cobertura ?? COBERTURA)),
  };
  const apiResumo = apiResumoFalsa(resumo);
  const navegar = vi.fn<(t: Tela) => void>();
  render(
    <VisaoGeral
      projeto={PROJETO}
      api={api}
      apiResumo={apiResumo}
      apiConfiguracoes={apiConfiguracoesFalsa()}
      apiMapa={{ mapa: () => Promise.resolve(extras.mapa ?? MAPA) }}
      navegar={navegar}
    />,
  );
  return { conhecimento, apiResumo, navegar };
}

const resumoCheio = (): ResumoProjeto =>
  resumoFicticio({
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
  it("lê o resumo uma vez e mostra o cabeçalho com origens e selos de saúde", async () => {
    const { apiResumo } = montar();
    expect(await screen.findByRole("heading", { name: "Faturamento Exemplo" })).toBeInTheDocument();
    await screen.findByRole("region", { name: "Próximos passos" });
    expect(apiResumo.resumoProjeto).toHaveBeenCalledTimes(1);
    expect(apiResumo.resumoProjeto).toHaveBeenCalledWith(PROJETO);
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
    expect(within(passos).getByText("Demanda D-0001 sem análise")).toBeInTheDocument();
    expect(within(passos).queryByText("Regras a revalidar")).not.toBeInTheDocument();
    expect(within(passos).getByRole("button", { name: "Ver as 4 atividades" })).toBeInTheDocument();

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
    fireEvent.click(within(passos).getByRole("button", { name: "Ver as demandas" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demandas", projeto: PROJETO });
    fireEvent.click(within(passos).getByRole("button", { name: "Ver as 4 atividades" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "inicio" });
  });

  it("os quatro números levam à lista correspondente", async () => {
    const { navegar } = montar(resumoCheio());
    const revisao = await screen.findByRole("button", { name: /^Aguardando sua revisão/ });
    expect(revisao).toHaveTextContent("1");
    fireEvent.click(revisao);
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "inicio" });
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
    fireEvent.click(within(sessoes).getByRole("button", { name: "Abrir a análise" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0002", abertura: { tipo: "reabrir" } },
    });
    const recentes = within(painel).getByRole("list", { name: "Atividade recente" });
    expect(recentes).toHaveTextContent("A-0008: o agente aguarda você");
    expect(recentes).toHaveTextContent("A-0003 concluída pelo agente");
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
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "analise", projeto: PROJETO });
  });

  it("o ambiente por preparar entra nos próximos passos", async () => {
    const config = apiConfiguracoesFalsa();
    config.verificarAmbiente.mockResolvedValue({ pronto: false, pendencias: ["servidor MCP"] });
    const conhecimento = apiConhecimentoFalsa();
    const navegar = vi.fn<(t: Tela) => void>();
    render(
      <VisaoGeral
        projeto={PROJETO}
        api={conhecimento.api}
        apiResumo={apiResumoFalsa(resumoVazio())}
        apiConfiguracoes={config}
        apiMapa={{ mapa: () => Promise.resolve(MAPA) }}
        navegar={navegar}
      />,
    );
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    fireEvent.click(await within(passos).findByRole("button", { name: "Preparar" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "configurar", projeto: PROJETO });
  });

  it("falha no resumo mostra o erro e deixa tentar de novo", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const apiResumo = apiResumoFalsa();
    apiResumo.resumoProjeto.mockRejectedValueOnce(new Error("disco cheio"));
    render(
      <VisaoGeral
        projeto={PROJETO}
        api={conhecimento.api}
        apiResumo={apiResumo}
        apiConfiguracoes={apiConfiguracoesFalsa()}
        apiMapa={{ mapa: () => Promise.resolve(MAPA) }}
        navegar={vi.fn()}
      />,
    );
    expect(await screen.findByText(/disco cheio/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    await screen.findByRole("region", { name: "Próximos passos" });
    expect(screen.queryByText(/disco cheio/)).not.toBeInTheDocument();
    expect(apiResumo.resumoProjeto).toHaveBeenCalledTimes(2);
  });

  it("relê o resumo quando a base muda", async () => {
    const { conhecimento, apiResumo } = montar();
    await screen.findByRole("region", { name: "Próximos passos" });
    expect(apiResumo.resumoProjeto).toHaveBeenCalledTimes(1);
    conhecimento.avisar();
    await waitFor(() => expect(apiResumo.resumoProjeto).toHaveBeenCalledTimes(2));
  });
});
