import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { FormRepositorios } from "./FormRepositorios";
import { ProntidaoRepositorios } from "./ProntidaoRepositorios";
import {
  REPOSITORIO_CODIGO_VAZIO,
  SEM_REPOSITORIOS,
  repositorioDaUrl,
  validarRepositorios,
  type RepositorioCodigo,
  type Repositorios,
} from "./repositorios";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { apiConfiguracoesFalsa, configuracoesFicticias } from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const JIRA: Repositorios["tracker"] = {
  kind: "jira",
  url: "https://jira.example.com",
  project_key: "PROJ",
  jql: "",
  active_sprint_only: true,
  max_issues: 100,
  credential_cmd: "pass show jira | head -1",
  config_file: "",
  epic_link_field: "",
  epic_name_field: "",
  sprint_field: "",
  type_map: { Epic: "epic" },
};

const GITHUB: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  caminho: "o/r",
  url_clone: "https://github.com/o/r.git",
};

const GITLAB: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  forja: "gitlab",
  host: "gitlab.example.com",
  caminho: "g/p/app",
  url_clone: "git@gitlab.example.com:g/p/app.git",
  token: "comando",
  comando_credencial: "pass show gitlab | head -1",
};

function Controlado({
  inicial = SEM_REPOSITORIOS,
  travado,
  aoMudar,
}: {
  inicial?: Repositorios;
  travado?: Repositorios;
  aoMudar?: (r: Repositorios) => void;
}) {
  const [v, setV] = useState(inicial);
  return (
    <FormRepositorios
      valor={v}
      travado={travado}
      aoMudar={(r) => {
        setV(r);
        aoMudar?.(r);
      }}
    />
  );
}

function ultimoValor(aoMudar: ReturnType<typeof vi.fn>): Repositorios {
  return aoMudar.mock.calls[aoMudar.mock.calls.length - 1]![0] as Repositorios;
}

describe("repositorioDaUrl", () => {
  it.each([
    ["https://github.com/o/r.git", "github", "github.com", "o/r"],
    ["https://github.com/o/r/", "github", "github.com", "o/r"],
    ["git@github.com:o/r.git", "github", "github.com", "o/r"],
    ["ssh://git@ssh.github.com:443/o/r.git", "github", "github.com", "o/r"],
    ["https://gitlab.example.com/g/sub/app.git", "gitlab", "gitlab.example.com", "g/sub/app"],
    ["git@gitlab.example.com:g/sub/app.git", "gitlab", "gitlab.example.com", "g/sub/app"],
    ["ssh://git@gitlab.example.com:2222/g/app", "gitlab", "gitlab.example.com", "g/app"],
  ])("%s", (url, forja, host, caminho) => {
    expect(repositorioDaUrl(url)).toMatchObject({
      forja,
      host,
      caminho,
      url_clone: url,
      token: "global",
      branch: "",
      pasta: null,
    });
  });

  it("devolve null quando não dá para deduzir", () => {
    expect(repositorioDaUrl("")).toBeNull();
    expect(repositorioDaUrl("https://github.com")).toBeNull();
    expect(repositorioDaUrl("https://github.com/o")).toBeNull();
    expect(repositorioDaUrl("nada a ver")).toBeNull();
  });
});

describe("validarRepositorios", () => {
  it("aceita sem repositórios e as combinações cruzadas", () => {
    expect(validarRepositorios(SEM_REPOSITORIOS)).toBeNull();
    const gitlab = {
      kind: "gitlab" as const,
      host: "gitlab.example.com",
      path: "g/p/app",
      clone_url: "",
      credential_cmd: "pass show gitlab",
    };
    expect(validarRepositorios({ codigo: [GITLAB], tracker: JIRA })).toBeNull();
    expect(validarRepositorios({ codigo: [GITHUB, GITLAB], tracker: gitlab })).toBeNull();
  });

  it("recusa repositórios de código inválidos, apontando qual", () => {
    const v = (...codigo: RepositorioCodigo[]) => validarRepositorios({ codigo, tracker: null });
    expect(v(GITHUB, { ...GITHUB, caminho: "O/R" })).toMatch(
      /^Repositório de código 2: repete o repositório de código 1/,
    );
    expect(v({ ...GITLAB, caminho: "app" })).toMatch(/código 1: caminho do GitLab/);
    expect(v({ ...GITLAB, host: " " })).toMatch(/código 1: informe o host/);
    expect(v(GITHUB, { ...GITHUB, caminho: "o/r/x" })).toMatch(/código 2: caminho do GitHub/);
    expect(v({ ...GITLAB, comando_credencial: " " })).toMatch(/código 1: informe o comando/);
    expect(v({ ...GITLAB, token: "global", comando_credencial: "" })).toBeNull();
  });

  it("recusa com as regras do Jira do ctower-agents", () => {
    const t = (p: object) =>
      validarRepositorios({ codigo: [], tracker: { ...JIRA, ...p } as never });
    expect(t({ url: "jira.example.com" })).toMatch(/URL do Jira/);
    expect(t({ url: "https://jira.example.com/" })).toMatch(/URL do Jira/);
    expect(t({ project_key: "proj" })).toMatch(/Chave do projeto/);
    expect(t({ credential_cmd: " " })).toMatch(/credencial/);
    expect(t({ active_sprint_only: false })).toMatch(/JQL/);
    expect(t({ active_sprint_only: false, jql: "a", max_issues: 5001 })).toMatch(/Limite/);
    expect(t({ active_sprint_only: false, jql: "a", max_issues: 5000 })).toBeNull();
  });
});

describe("FormRepositorios", () => {
  it("monta código no GitLab e issues no Jira, com sprint ativa por padrão", () => {
    const aoMudar = vi.fn();
    render(<Controlado aoMudar={aoMudar} />);
    fireEvent.click(screen.getByRole("button", { name: "Adicionar repositório" }));
    fireEvent.change(screen.getByLabelText("Forja"), { target: { value: "gitlab" } });
    fireEvent.change(screen.getByLabelText(/Repositório de demandas/), {
      target: { value: "jira" },
    });
    expect(screen.getByLabelText(/Usar somente a sprint ativa/)).toBeChecked();
    expect(screen.queryByLabelText("JQL")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("URL do Jira"), {
      target: { value: "https://jira.example.com" },
    });
    fireEvent.change(screen.getByLabelText("Chave do projeto"), {
      target: { value: "PROJ" },
    });
    const ultimo = ultimoValor(aoMudar);
    expect(ultimo.codigo[0]?.forja).toBe("gitlab");
    expect(ultimo.tracker).toMatchObject({
      kind: "jira",
      url: "https://jira.example.com",
      project_key: "PROJ",
      active_sprint_only: true,
      type_map: { Epic: "epic", Bug: "bug" },
    });
    // Fora do modo sprint ativa, pede o JQL e o limite.
    fireEvent.click(screen.getByLabelText(/Usar somente a sprint ativa/));
    expect(screen.getByLabelText("JQL")).toBeInTheDocument();
    expect(screen.getByLabelText(/Máximo de issues/)).toHaveValue(100);
  });

  it("adiciona um repositório do GitHub e um do GitLab pela URL de clone", () => {
    const aoMudar = vi.fn();
    render(<Controlado aoMudar={aoMudar} />);
    const adicionar = screen.getByRole("button", { name: "Adicionar repositório" });
    fireEvent.click(adicionar);
    fireEvent.click(adicionar);
    const um = within(screen.getByRole("group", { name: "Repositório de código 1" }));
    const dois = within(screen.getByRole("group", { name: "Repositório de código 2" }));
    fireEvent.change(um.getByLabelText(/URL de clone/), {
      target: { value: "https://github.com/o/r.git" },
    });
    fireEvent.change(dois.getByLabelText(/URL de clone/), {
      target: { value: "git@gitlab.example.com:g/p/app.git" },
    });
    expect(dois.getByLabelText("Forja")).toHaveValue("gitlab");
    expect(dois.queryByLabelText(/Comando de credencial/)).not.toBeInTheDocument();
    fireEvent.change(dois.getByLabelText("Token"), { target: { value: "comando" } });
    fireEvent.change(dois.getByLabelText(/Comando de credencial/), {
      target: { value: "pass show gitlab | head -1" },
    });
    fireEvent.change(dois.getByLabelText("Branch (opcional)"), { target: { value: "main" } });
    const ultimo = ultimoValor(aoMudar);
    expect(ultimo.codigo).toEqual([
      GITHUB,
      { ...GITLAB, branch: "main", url_clone: "git@gitlab.example.com:g/p/app.git" },
    ]);
    expect(validarRepositorios(ultimo)).toBeNull();
    // O item novo sai da lista.
    fireEvent.click(um.getByRole("button", { name: "Remover" }));
    expect(ultimoValor(aoMudar).codigo).toHaveLength(1);
    expect(ultimoValor(aoMudar).codigo[0]?.forja).toBe("gitlab");
  });

  it("repositório já gravado não sai nem muda de forja, host ou caminho", () => {
    const gravado: Repositorios = { codigo: [GITHUB], tracker: null };
    render(<Controlado inicial={gravado} travado={gravado} />);
    fireEvent.click(screen.getByRole("button", { name: "Adicionar repositório" }));
    const um = within(screen.getByRole("group", { name: "Repositório de código 1" }));
    const dois = within(screen.getByRole("group", { name: "Repositório de código 2" }));
    expect(um.queryByRole("button", { name: "Remover" })).not.toBeInTheDocument();
    expect(um.getByLabelText("Forja")).toBeDisabled();
    expect(um.getByLabelText("Host")).toBeDisabled();
    expect(um.getByLabelText("Caminho")).toBeDisabled();
    expect(um.getByLabelText("Branch (opcional)")).toBeEnabled();
    expect(dois.getByRole("button", { name: "Remover" })).toBeInTheDocument();
    expect(dois.getByLabelText("Caminho")).toBeEnabled();
  });

  it("oferece GitLab Issues como rastreador", () => {
    render(<Controlado />);
    const seletor = screen.getByLabelText(/Repositório de demandas/);
    expect(within(seletor).getByRole("option", { name: "GitLab Issues" })).toBeInTheDocument();
    expect(within(seletor).getByRole("option", { name: "Jira" })).toBeInTheDocument();
  });
});

describe("ProntidaoRepositorios", () => {
  it("mostra cada passo e a orientação de quem falhou", async () => {
    const api = {
      prontidaoRepositorios: vi.fn().mockResolvedValue({
        integracoes: [
          {
            titulo: "Issues (Jira)",
            passos: [
              {
                id: "cli",
                rotulo: "jira-cli instalado",
                situacao: "ok",
                detalhe: "jira version 1.7.0",
                orientacao: null,
              },
              {
                id: "servidor",
                rotulo: "Servidor igual à URL",
                situacao: "falhou",
                detalhe: "a configuração usa outro",
                orientacao: "Corrija o `server` do .config.yml.",
              },
              {
                id: "me",
                rotulo: "Autenticação (jira me)",
                situacao: "pulado",
                detalhe: null,
                orientacao: null,
              },
            ],
          },
        ],
      }),
    };
    render(<ProntidaoRepositorios api={api} projeto="p" />);
    fireEvent.click(screen.getByRole("button", { name: "Conferir" }));
    expect(await screen.findByText("jira-cli instalado")).toBeInTheDocument();
    expect(screen.getByText("Corrija o `server` do .config.yml.")).toBeInTheDocument();
    expect(screen.getByText("falhou")).toHaveAttribute("data-tone", "error");
    expect(screen.getByText("não conferido")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Conferir de novo" })).toBeInTheDocument();
  });
});

describe("repositórios nas configurações", () => {
  it("grava os repositórios do projeto e trava o tipo depois", async () => {
    const api = apiConfiguracoesFalsa();
    render(<TelaConfiguracoes api={api} aoMudarProjetos={vi.fn()} />);
    const grupo = within(
      await screen.findByRole("group", {
        name: "Repositórios de Faturamento Exemplo",
      }),
    );
    fireEvent.click(grupo.getByRole("button", { name: "Adicionar repositório" }));
    fireEvent.change(grupo.getByLabelText(/URL de clone/), {
      target: { value: "https://github.com/o/r.git" },
    });
    fireEvent.click(grupo.getByRole("button", { name: "Salvar repositórios" }));
    await waitFor(() =>
      expect(api.salvarRepositorios).toHaveBeenCalledWith("faturamento-exemplo", {
        codigo: [GITHUB],
        tracker: null,
      }),
    );
  });

  it("não envia cadastro inválido", async () => {
    const api = apiConfiguracoesFalsa();
    render(<TelaConfiguracoes api={api} aoMudarProjetos={vi.fn()} />);
    const grupo = within(
      await screen.findByRole("group", {
        name: "Repositórios de Faturamento Exemplo",
      }),
    );
    fireEvent.change(grupo.getByLabelText(/Repositório de demandas/), {
      target: { value: "jira" },
    });
    fireEvent.click(grupo.getByRole("button", { name: "Salvar repositórios" }));
    expect(await grupo.findByRole("alert")).toHaveTextContent(/URL do Jira/);
    expect(api.salvarRepositorios).not.toHaveBeenCalled();
  });

  it("com o cadastro gravado, código e rastreador ficam travados", async () => {
    const cfg = configuracoesFicticias();
    cfg.projetos[0].repositorios = { codigo: [GITHUB], tracker: JIRA };
    render(<TelaConfiguracoes api={apiConfiguracoesFalsa(cfg)} aoMudarProjetos={vi.fn()} />);
    const grupo = within(
      await screen.findByRole("group", {
        name: "Repositórios de Faturamento Exemplo",
      }),
    );
    expect(grupo.getByLabelText("Forja")).toBeDisabled();
    expect(grupo.queryByRole("button", { name: "Remover" })).not.toBeInTheDocument();
    expect(grupo.getByLabelText(/Repositório de demandas/)).toBeDisabled();
  });
});
