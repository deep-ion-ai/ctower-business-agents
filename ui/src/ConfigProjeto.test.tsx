import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ConfigProjeto, type PropsConfigProjeto } from "./ConfigProjeto";
import type { AmbienteDetalhado, ProjetoConfig } from "./configuracoes";
import {
  REPOSITORIO_CODIGO_VAZIO,
  type Prontidao,
  type RepositorioCodigo,
  type TrackerConfig,
} from "./repositorios";
import type { EstadoDoProjeto } from "./saudeProjeto";
import {
  BASE,
  CODIGO,
  TEXTOS,
  apiConfiguracoesFalsa,
  configuracoesFicticias,
} from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PRONTO: AmbienteDetalhado = {
  pronto: true,
  pendencias: [],
  verificacao: {
    diretorio: CODIGO,
    chave: CODIGO,
    servidorMcp: "ok",
    pastaConfiavel: true,
    hooks: [
      { evento: "SessionStart", situacao: "ok" },
      { evento: "Stop", situacao: "ok" },
    ],
    hooksDesativados: false,
    pronto: true,
  },
};

const SEM_PREPARO: AmbienteDetalhado = {
  pronto: false,
  pendencias: ["servidor MCP ausente", "hooks ausentes"],
  verificacao: {
    diretorio: CODIGO,
    chave: CODIGO,
    servidorMcp: "ausente",
    pastaConfiavel: false,
    hooks: [
      { evento: "SessionStart", situacao: "ausente" },
      { evento: "Stop", situacao: "ausente" },
    ],
    hooksDesativados: false,
    pronto: false,
  },
};

const API_REPO: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  caminho: "empresa/faturamento-api",
  url_clone: "https://github.com/empresa/faturamento-api.git",
  branch: "main",
  pasta: CODIGO,
};

const WEB_REPO: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  forja: "gitlab",
  host: "gitlab.empresa.com",
  caminho: "fin/faturamento-web",
  url_clone: "git@gitlab.empresa.com:fin/faturamento-web.git",
  token: "proprio",
};

const JIRA: TrackerConfig = {
  kind: "jira",
  url: "https://jira.example.com",
  project_key: "FAT",
  jql: "",
  active_sprint_only: true,
  max_issues: 100,
  credential_cmd: "",
  config_file: "",
  epic_link_field: "",
  epic_name_field: "",
  sprint_field: "",
  type_map: { Epic: "epic" },
};

const ok = (titulo: string) => ({
  titulo,
  passos: [
    { id: "acesso", rotulo: "Acesso", situacao: "ok" as const, detalhe: null, orientacao: null },
  ],
});

const recusado = (titulo: string) => ({
  titulo,
  passos: [
    {
      id: "acesso",
      rotulo: "Acesso de leitura (git ls-remote)",
      situacao: "falhou" as const,
      detalhe: "token recusado",
      orientacao: "Confira se o token tem permissão de leitura.",
    },
  ],
});

function projetoFicticio(parcial: Partial<ProjetoConfig> = {}): ProjetoConfig {
  return { ...configuracoesFicticias().projetos[0], ...parcial };
}

function montar({
  projeto = projetoFicticio(),
  estado = { ambiente: PRONTO, prontidao: { integracoes: [] } },
  api = apiConfiguracoesFalsa(),
}: {
  projeto?: ProjetoConfig;
  estado?: EstadoDoProjeto;
  api?: ReturnType<typeof apiConfiguracoesFalsa>;
} = {}) {
  const props: PropsConfigProjeto = {
    api,
    cfg: configuracoesFicticias(),
    projeto,
    textos: TEXTOS,
    estado,
    aoMudar: vi.fn(),
    aoAbrirTexto: vi.fn(),
    aoEditar: vi.fn(),
    aoReverificarAmbiente: vi.fn(() => Promise.resolve(PRONTO)),
    aoConferirProntidao: vi.fn(),
    aoIrParaAcessos: vi.fn(),
    aoPendencia: vi.fn(),
  };
  const vista = render(<ConfigProjeto {...props} />);
  const aba = (nome: string | RegExp) => {
    fireEvent.click(screen.getByRole("tab", { name: nome }));
    return within(screen.getByRole("tabpanel"));
  };
  return { ...props, api, aba, ...vista };
}

describe("ConfigProjeto", () => {
  describe("cabeçalho e abas", () => {
    it("mostra nome, selo, resumo e as cinco abas com aria de abas", () => {
      montar();
      expect(screen.getByRole("heading", { name: "Faturamento Exemplo" })).toBeInTheDocument();
      expect(screen.getByText("✓ Tudo pronto")).toBeInTheDocument();
      expect(
        screen.getByText("1 pasta · 0 repositórios de código · sem demandas conectadas"),
      ).toBeInTheDocument();
      const abas = within(screen.getByRole("tablist", { name: "Partes do projeto" })).getAllByRole(
        "tab",
      );
      expect(abas.map((a) => a.textContent)).toEqual([
        "Visão geral",
        "Código e demandas",
        "O que é analisado",
        "Agente",
        "Avançado",
      ]);
      const painel = screen.getByRole("tabpanel");
      expect(painel).toHaveAttribute("aria-labelledby", abas[0].id);
      expect(abas[0]).toHaveAttribute("aria-controls", painel.id);
      expect(abas[0]).toHaveAttribute("tabindex", "0");
      expect(abas[1]).toHaveAttribute("tabindex", "-1");
    });

    it("anda pelas abas com as setas, Home e End", () => {
      montar();
      const geral = screen.getByRole("tab", { name: "Visão geral" });
      geral.focus();
      fireEvent.keyDown(geral, { key: "ArrowRight" });
      const codigo = screen.getByRole("tab", { name: "Código e demandas" });
      expect(codigo).toHaveAttribute("aria-selected", "true");
      expect(codigo).toHaveFocus();
      fireEvent.keyDown(codigo, { key: "End" });
      expect(screen.getByRole("tab", { name: "Avançado" })).toHaveFocus();
      fireEvent.keyDown(screen.getByRole("tab", { name: "Avançado" }), { key: "ArrowRight" });
      expect(screen.getByRole("tab", { name: "Visão geral" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      fireEvent.keyDown(screen.getByRole("tab", { name: "Visão geral" }), { key: "ArrowLeft" });
      expect(screen.getByRole("tab", { name: "Avançado" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      fireEvent.keyDown(screen.getByRole("tab", { name: "Avançado" }), { key: "Home" });
      expect(screen.getByRole("tab", { name: "Visão geral" })).toHaveFocus();
    });

    it("confere a prontidão dos repositórios ao abrir", () => {
      const { aoConferirProntidao } = montar();
      expect(aoConferirProntidao).toHaveBeenCalledWith("faturamento-exemplo");
    });
  });

  describe("Visão geral", () => {
    it("lista a saúde e marca as abas com problema", () => {
      montar({
        projeto: projetoFicticio({ repositorios: { codigo: [API_REPO], tracker: null } }),
        estado: { ambiente: SEM_PREPARO, prontidao: { integracoes: [recusado("Código")] } },
      });
      expect(screen.getByText("2 pendências")).toBeInTheDocument();
      expect(screen.getByText("Um repositório está sem acesso")).toBeInTheDocument();
      expect(screen.getByText("Agente ainda não preparado")).toBeInTheDocument();
      expect(screen.getByText("Demandas não conectadas")).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Visão geral/ })).toHaveTextContent("2");
      expect(screen.getByRole("tab", { name: /Código e demandas/ })).toHaveTextContent("!");
      expect(screen.getByRole("tab", { name: /Agente/ })).toHaveTextContent("!");
      expect(screen.getByRole("tab", { name: "O que é analisado" })).not.toHaveTextContent("!");
    });

    it("cada item leva à aba certa", () => {
      montar({ estado: { ambiente: SEM_PREPARO, prontidao: { integracoes: [] } } });
      fireEvent.click(screen.getByRole("button", { name: "Preparar: Agente ainda não preparado" }));
      expect(screen.getByRole("tab", { name: /Agente/ })).toHaveAttribute("aria-selected", "true");
      fireEvent.click(screen.getByRole("tab", { name: /Visão geral/ }));
      fireEvent.click(screen.getByRole("button", { name: "Ajustar: O que o agente lê" }));
      expect(screen.getByRole("tab", { name: "O que é analisado" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    });

    it("uma pasta que sumiu é pendência de O que é analisado", () => {
      montar({
        projeto: projetoFicticio({
          pastas: [{ caminho: CODIGO, prefixo: null, existe: false }],
        }),
      });
      expect(screen.getByText("Uma pasta não existe mais")).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /O que é analisado/ })).toHaveTextContent("!");
    });

    it("enquanto o ambiente não foi verificado, o selo diz que está verificando", () => {
      montar({ estado: {} });
      expect(screen.getByText("Verificando…")).toBeInTheDocument();
      expect(screen.getByText("Verificando o agente…")).toBeInTheDocument();
    });
  });

  describe("Código e demandas", () => {
    const comRepos = (prontidao: Prontidao | undefined, tracker: TrackerConfig | null = JIRA) =>
      montar({
        projeto: projetoFicticio({ repositorios: { codigo: [API_REPO, WEB_REPO], tracker } }),
        estado: { ambiente: PRONTO, prontidao },
      });

    it("mostra forja, endereço, branch, pasta, tipo de token e o acesso conferido", () => {
      const { aba } = comRepos({
        integracoes: [ok("Código api"), recusado("Código web"), ok("Jira")],
      });
      const painel = aba(/Código e demandas/);
      const linhas = within(
        painel.getByRole("region", { name: "Repositórios de código" }),
      ).getAllByRole("listitem");
      expect(linhas[0]).toHaveTextContent("GitHub");
      expect(linhas[0]).toHaveTextContent("https://github.com/empresa/faturamento-api.git");
      expect(linhas[0]).toHaveTextContent("Branch main");
      expect(linhas[0]).toHaveTextContent(`pasta ${CODIGO}`);
      expect(linhas[0]).toHaveTextContent("token global de github.com");
      expect(linhas[0]).toHaveTextContent("✓ acesso ok");
      expect(linhas[1]).toHaveTextContent("token próprio deste repositório");
      expect(linhas[1]).toHaveTextContent("✕ sem acesso");
      expect(linhas[1]).toHaveTextContent("token recusado");
      expect(linhas[1]).toHaveTextContent("Confira se o token tem permissão de leitura.");
      // Token próprio: a ação é editar o repositório, não ir para Acessos.
      expect(
        within(linhas[1]).getByRole("button", { name: "Editar fin/faturamento-web" }),
      ).toBeInTheDocument();
      expect(
        painel.getByText(/Endereço e forja não mudam depois de adicionados/),
      ).toBeInTheDocument();
      expect(painel.queryByRole("button", { name: "Conferir" })).not.toBeInTheDocument();
    });

    it("com o token global recusado, Trocar o token leva a Acessos", () => {
      const { aba, aoIrParaAcessos } = comRepos({
        integracoes: [recusado("Código api"), ok("Código web"), ok("Jira")],
      });
      const painel = aba(/Código e demandas/);
      fireEvent.click(painel.getByRole("button", { name: "Trocar o token global de github.com" }));
      expect(aoIrParaAcessos).toHaveBeenCalled();
    });

    it("enquanto confere, diz que está conferindo", () => {
      const { aba } = comRepos(undefined);
      const painel = aba("Código e demandas");
      expect(painel.getAllByText("conferindo…").length).toBeGreaterThan(0);
      expect(painel.getByRole("button", { name: "Conferir de novo" })).toBeDisabled();
    });

    it("edita a branch de um repositório com Cancelar à esquerda de Salvar", async () => {
      const { aba, api, aoMudar, aoConferirProntidao } = comRepos({ integracoes: [] });
      const painel = aba("Código e demandas");
      fireEvent.click(painel.getByRole("button", { name: "Editar empresa/faturamento-api" }));
      expect(painel.getByLabelText("Forja")).toBeDisabled();
      const [cancelar, salvar] = painel
        .getAllByRole("button")
        .filter((b) => b.textContent === "Cancelar" || b.textContent === "Salvar");
      expect(cancelar).toHaveTextContent("Cancelar");
      expect(salvar).toHaveTextContent("Salvar");
      fireEvent.change(painel.getByLabelText(/Branch/), { target: { value: "develop" } });
      fireEvent.click(salvar);
      await waitFor(() =>
        expect(api.salvarRepositorios).toHaveBeenCalledWith("faturamento-exemplo", {
          codigo: [{ ...API_REPO, branch: "develop" }, WEB_REPO],
          tracker: JIRA,
        }),
      );
      expect(aoMudar).toHaveBeenCalled();
      expect(aoConferirProntidao).toHaveBeenCalledWith("faturamento-exemplo", true);
    });

    it("demandas: origem em pílulas, campos essenciais, Mais opções e Testar conexão", async () => {
      const { aba, api } = comRepos({ integracoes: [ok("a"), ok("b"), ok("Jira")] }, null);
      api.testarJira.mockResolvedValue({
        titulo: "Jira",
        passos: [
          {
            id: "me",
            rotulo: "Autenticação (jira me)",
            situacao: "ok",
            detalhe: null,
            orientacao: null,
          },
        ],
      });
      const painel = aba("Código e demandas");
      const origens = within(painel.getByRole("radiogroup", { name: "Origem das demandas" }));
      expect(origens.getByRole("radio", { name: "Nenhuma" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
      fireEvent.click(origens.getByRole("radio", { name: "Jira" }));
      expect(painel.getByLabelText("URL do Jira")).toBeInTheDocument();
      expect(painel.getByLabelText("Chave do projeto")).toBeInTheDocument();
      // JQL, campos e tipos ficam em Mais opções.
      const mais = painel.getByText("Mais opções (JQL, campos, tipos)").closest("details")!;
      expect(mais).not.toHaveAttribute("open");
      expect(within(mais).getByLabelText(/Tipos de issue/)).toBeInTheDocument();

      fireEvent.change(painel.getByLabelText("URL do Jira"), {
        target: { value: "https://jira.example.com" },
      });
      fireEvent.click(painel.getByRole("button", { name: "Testar conexão" }));
      await waitFor(() => expect(api.testarJira).toHaveBeenCalled());
      expect(await painel.findByText("Autenticação (jira me)")).toBeInTheDocument();

      // As setas andam pelas pílulas.
      fireEvent.keyDown(origens.getByRole("radio", { name: "Jira" }), { key: "ArrowRight" });
      expect(origens.getByRole("radio", { name: "Nenhuma" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
      expect(painel.queryByRole("button", { name: "Salvar as demandas" })).not.toBeInTheDocument();
    });

    it("com a origem salva, as outras pílulas ficam travadas", () => {
      const { aba } = comRepos({ integracoes: [ok("a"), ok("b"), ok("Jira")] });
      const painel = aba("Código e demandas");
      expect(painel.getByText("✓ conectado")).toBeInTheDocument();
      expect(painel.getByRole("radio", { name: "Jira" })).toHaveAttribute("aria-checked", "true");
      expect(painel.getByRole("radio", { name: "GitHub Issues" })).toBeDisabled();
      expect(painel.getByRole("radio", { name: "Nenhuma" })).toBeDisabled();
    });
  });

  describe("O que é analisado", () => {
    it("mostra o resumo com os segredos em destaque e a lista para reincluir", async () => {
      const { aba, api } = montar();
      const painel = aba("O que é analisado");
      await waitFor(() =>
        expect(api.previaLeitura).toHaveBeenCalledWith({
          projeto: "faturamento-exemplo",
          pastas: [CODIGO],
          exclusoes: ["docs/antigos/"],
          inclusoes: [],
        }),
      );
      expect(await painel.findByText("14 arquivos")).toBeInTheDocument();
      expect(painel.getByText("1 itens")).toBeInTheDocument();
      expect(painel.getByText(/1 arquivo parece ter segredo/)).toHaveClass("attention-text");
      fireEvent.click(painel.getByRole("button", { name: "Ver a lista e reincluir itens" }));
      fireEvent.click(painel.getByRole("button", { name: "Incluir .env na análise" }));
      await waitFor(() =>
        expect(api.previaLeitura).toHaveBeenLastCalledWith(
          expect.objectContaining({ inclusoes: ["/.env"] }),
        ),
      );
      expect(painel.getByRole("region", { name: "Alterações não salvas" })).toBeInTheDocument();
    });

    it("adiciona uma pasta, não deixa remover a última e salva relendo o projeto", async () => {
      const { aba, api, aoMudar, aoReverificarAmbiente, aoPendencia } = montar();
      api.escolherPasta.mockResolvedValue("/codigo/telas");
      const painel = aba("O que é analisado");
      const pastas = within(painel.getByRole("region", { name: "Pastas do projeto" }));
      expect(pastas.getByRole("button", { name: `Remover a pasta ${CODIGO}` })).toBeDisabled();
      expect(pastas.getByText("raiz (sem prefixo)")).toBeInTheDocument();

      fireEvent.click(pastas.getByRole("button", { name: "+ Adicionar pasta" }));
      expect(await pastas.findByText("/codigo/telas")).toBeInTheDocument();
      expect(pastas.getByText("nova")).toBeInTheDocument();
      expect(pastas.getByRole("button", { name: `Remover a pasta ${CODIGO}` })).toBeEnabled();
      await waitFor(() =>
        expect(api.previaLeitura).toHaveBeenLastCalledWith(
          expect.objectContaining({ pastas: [CODIGO, "/codigo/telas"] }),
        ),
      );
      await waitFor(() => expect(aoPendencia).toHaveBeenLastCalledWith(true));

      const barra = within(painel.getByRole("region", { name: "Alterações não salvas" }));
      const botoes = barra.getAllByRole("button").map((b) => b.textContent);
      expect(botoes).toEqual(["Descartar", "Salvar e ler o projeto de novo"]);
      fireEvent.click(barra.getByRole("button", { name: "Salvar e ler o projeto de novo" }));
      await waitFor(() =>
        expect(api.salvarLeitura).toHaveBeenCalledWith({
          projeto: "faturamento-exemplo",
          pastas: [CODIGO, "/codigo/telas"],
          exclusoes: ["docs/antigos/"],
          inclusoes: [],
        }),
      );
      expect(aoMudar).toHaveBeenCalled();
      expect(aoReverificarAmbiente).toHaveBeenCalledWith("faturamento-exemplo");
    });

    it("exclusões em chips: remove e adiciona; Descartar volta ao salvo", async () => {
      const { aba, api } = montar();
      const painel = aba("O que é analisado");
      fireEvent.click(painel.getByRole("button", { name: "Remover a exclusão docs/antigos/" }));
      fireEvent.change(painel.getByLabelText("Padrão para deixar de fora"), {
        target: { value: "**/legado/**" },
      });
      fireEvent.click(painel.getByRole("button", { name: "Adicionar" }));
      await waitFor(() =>
        expect(api.previaLeitura).toHaveBeenLastCalledWith(
          expect.objectContaining({ exclusoes: ["**/legado/**"] }),
        ),
      );
      fireEvent.click(painel.getByRole("button", { name: "Descartar" }));
      expect(painel.getByText("docs/antigos/")).toBeInTheDocument();
      expect(
        painel.queryByRole("region", { name: "Alterações não salvas" }),
      ).not.toBeInTheDocument();
      expect(api.salvarLeitura).not.toHaveBeenCalled();
    });

    it("trocar de aba com alterações não salvas pede confirmação", () => {
      const { aba } = montar();
      const painel = aba("O que é analisado");
      fireEvent.click(painel.getByRole("button", { name: "Remover a exclusão docs/antigos/" }));
      fireEvent.click(screen.getByRole("tab", { name: "Agente" }));
      const dialogo = within(screen.getByRole("dialog", { name: "Sair sem salvar?" }));
      const botoes = dialogo.getAllByRole("button").map((b) => b.textContent);
      expect(botoes).toEqual(["Cancelar", "Descartar as alterações"]);
      expect(dialogo.getByRole("button", { name: "Descartar as alterações" })).toHaveClass(
        "btn-danger",
      );
      fireEvent.click(dialogo.getByRole("button", { name: "Cancelar" }));
      expect(screen.getByRole("tab", { name: "O que é analisado" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      fireEvent.click(screen.getByRole("tab", { name: "Agente" }));
      fireEvent.click(screen.getByRole("button", { name: "Descartar as alterações" }));
      expect(screen.getByRole("tab", { name: "Agente" })).toHaveAttribute("aria-selected", "true");
    });

    it("a falha ao salvar aparece e as alterações continuam", async () => {
      const { aba, api } = montar();
      api.salvarLeitura.mockRejectedValueOnce(
        "o projeto precisa de ao menos uma pasta: não dá para remover a última",
      );
      const painel = aba("O que é analisado");
      fireEvent.click(painel.getByRole("button", { name: "Remover a exclusão docs/antigos/" }));
      fireEvent.click(painel.getByRole("button", { name: "Salvar e ler o projeto de novo" }));
      expect(await painel.findByRole("alert")).toHaveTextContent(/não dá para remover a última/);
      expect(painel.getByRole("region", { name: "Alterações não salvas" })).toBeInTheDocument();
    });
  });

  describe("Agente", () => {
    it("fala do preparo em linguagem de negócio, com os detalhes técnicos recolhidos", () => {
      const { aba } = montar();
      const painel = aba("Agente");
      expect(painel.getByText("✓ preparado")).toBeInTheDocument();
      expect(painel.getByText("Ferramentas do app conectadas ao Claude")).toBeInTheDocument();
      expect(painel.getByText("Avisos automáticos ligados")).toBeInTheDocument();
      const detalhes = painel.getByText("Detalhes técnicos").closest("details")!;
      expect(detalhes).not.toHaveAttribute("open");
      expect(within(detalhes).getByText("Servidor MCP “cba”: registrado")).toBeInTheDocument();
      expect(within(detalhes).getByText(/Hooks: 2 de 2/)).toBeInTheDocument();
    });

    it("sem preparo, oferece preparar sem a caixa da proteção (que tem lugar próprio)", async () => {
      const { aba, api, aoReverificarAmbiente } = montar({
        estado: { ambiente: SEM_PREPARO, prontidao: { integracoes: [] } },
      });
      const painel = aba(/Agente/);
      expect(painel.getByText("Ferramentas do app não conectadas")).toBeInTheDocument();
      fireEvent.click(painel.getByRole("button", { name: "Ver o que será alterado" }));
      const concordo = await painel.findByRole("button", { name: "Concordo, preparar o ambiente" });
      expect(painel.queryByRole("checkbox")).not.toBeInTheDocument();
      fireEvent.click(concordo);
      await waitFor(() => expect(api.prepararAmbiente).toHaveBeenCalled());
      await waitFor(() =>
        expect(aoReverificarAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
      );
      expect(api.mudarProtecao).not.toHaveBeenCalled();
    });

    it("a proteção é um interruptor que mostra as regras antes de ligar", async () => {
      const { aba, api } = montar();
      const painel = aba("Agente");
      const interruptor = await painel.findByRole("switch", {
        name: "Impedir que o agente altere arquivos",
      });
      await waitFor(() => expect(interruptor).toBeEnabled());
      expect(interruptor).toHaveAttribute("aria-checked", "false");
      fireEvent.click(interruptor);
      const confirmar = within(painel.getByRole("region", { name: "Confirmar a proteção" }));
      expect(confirmar.getByText(`Edit(/${CODIGO}/**)`)).toBeInTheDocument();
      expect(confirmar.getAllByRole("button").map((b) => b.textContent)).toEqual([
        "Cancelar",
        "Ligar a proteção",
      ]);
      expect(api.mudarProtecao).not.toHaveBeenCalled();
      fireEvent.click(confirmar.getByRole("button", { name: "Ligar a proteção" }));
      await waitFor(() =>
        expect(api.mudarProtecao).toHaveBeenCalledWith("faturamento-exemplo", true),
      );
      await waitFor(() => expect(interruptor).toHaveAttribute("aria-checked", "true"));
      expect(painel.getByText(/Ligada: o Claude não edita/)).toBeInTheDocument();
    });

    it("instruções do projeto com prévia, Editar e abrir no computador", async () => {
      const { aba, api, aoEditar } = montar();
      const painel = aba("Agente");
      const instrucoes = within(
        painel.getByRole("region", { name: "Instruções do projeto Faturamento Exemplo" }),
      );
      expect(await instrucoes.findByLabelText("Prévia das instruções")).toHaveTextContent(
        "Instrução: use {{id}} e {{titulo}}.",
      );
      fireEvent.click(
        instrucoes.getByRole("button", { name: "Editar Regras do projeto Faturamento Exemplo" }),
      );
      expect(aoEditar).toHaveBeenCalledWith("faturamento-exemplo/regras-do-projeto.md");
      fireEvent.click(
        instrucoes.getByRole("button", {
          name: "Abrir Regras do projeto Faturamento Exemplo no editor do computador",
        }),
      );
      await waitFor(() =>
        expect(api.abrirTexto).toHaveBeenCalledWith("faturamento-exemplo/regras-do-projeto.md"),
      );
    });
  });

  describe("Avançado", () => {
    it("desfaz o preparo só depois do diálogo, com botão de perigo", async () => {
      const { aba, api, aoReverificarAmbiente } = montar();
      const painel = aba("Avançado");
      fireEvent.click(painel.getByRole("button", { name: "Desfazer o preparo…" }));
      const dialogo = within(screen.getByRole("dialog", { name: "Desfazer o preparo do agente?" }));
      expect(dialogo.getAllByRole("button").map((b) => b.textContent)).toEqual([
        "Cancelar",
        "Desfazer o preparo",
      ]);
      expect(dialogo.getByRole("button", { name: "Cancelar" })).toHaveFocus();
      expect(api.desfazerAmbiente).not.toHaveBeenCalled();
      fireEvent.click(dialogo.getByRole("button", { name: "Desfazer o preparo" }));
      await waitFor(() => expect(api.desfazerAmbiente).toHaveBeenCalledWith("faturamento-exemplo"));
      expect(await painel.findByText(/Preparo desfeito/)).toHaveTextContent(
        "voltou a ser a de antes do preparo",
      );
      expect(aoReverificarAmbiente).toHaveBeenCalledWith("faturamento-exemplo");
    });

    it("não diz que voltou ao que era quando algo ficou, e diz o quê e por quê (#213)", async () => {
      const { aba, api } = montar();
      const ficou =
        "A pasta /codigo continua registrada em /home/ana/.claude.json com dados que o próprio Claude Code gravou ao abrir conversas nela (como o histórico de uso).";
      api.desfazerAmbiente.mockResolvedValueOnce({
        protecaoTirada: [],
        preparo: { diretorio: "/codigo", alteracoes: [], gravado: true },
        avisos: [ficou],
        igualAoAntes: false,
      });
      const painel = aba("Avançado");
      fireEvent.click(painel.getByRole("button", { name: "Desfazer o preparo…" }));
      const dialogo = within(screen.getByRole("dialog", { name: "Desfazer o preparo do agente?" }));
      fireEvent.click(dialogo.getByRole("button", { name: "Desfazer o preparo" }));
      const status = await painel.findByRole("status");
      expect(status).toHaveTextContent("não voltou inteira ao que era antes");
      expect(status).not.toHaveTextContent("voltou a ser a de antes");
      expect(within(status).getByText(ficou)).toBeInTheDocument();
    });

    it("sem preparo, não oferece desfazer", () => {
      const { aba } = montar({ estado: { ambiente: SEM_PREPARO, prontidao: { integracoes: [] } } });
      const painel = aba(/Avançado/);
      expect(painel.queryByRole("button", { name: "Desfazer o preparo…" })).not.toBeInTheDocument();
      expect(painel.getByRole("button", { name: "Descadastrar…" })).toHaveClass("btn-danger");
    });

    it("descadastra só digitando o nome, dizendo o que some e o que continua", async () => {
      const { aba, api, aoMudar } = montar();
      const painel = aba("Avançado");
      fireEvent.click(painel.getByRole("button", { name: "Descadastrar…" }));
      const dialogo = within(
        screen.getByRole("dialog", { name: "Descadastrar “Faturamento Exemplo”?" }),
      );
      expect(dialogo.getByText(/deixam de existir no app/)).toBeInTheDocument();
      expect(dialogo.getByText(`${BASE}/faturamento-exemplo`)).toBeInTheDocument();
      const botao = dialogo.getByRole("button", { name: "Descadastrar o projeto" });
      expect(botao).toHaveClass("btn-danger");
      const campo = dialogo.getByLabelText(/digite o nome do projeto/);
      fireEvent.change(campo, { target: { value: "Faturamento" } });
      expect(botao).toBeDisabled();
      fireEvent.change(campo, { target: { value: "Faturamento Exemplo" } });
      fireEvent.click(botao);
      await waitFor(() => expect(api.descadastrar).toHaveBeenCalledWith("faturamento-exemplo"));
      expect(aoMudar).toHaveBeenCalled();
    });

    it("Esc fecha o diálogo sem descadastrar", () => {
      const { aba, api } = montar();
      const painel = aba("Avançado");
      fireEvent.click(painel.getByRole("button", { name: "Descadastrar…" }));
      fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(api.descadastrar).not.toHaveBeenCalled();
    });
  });
});
