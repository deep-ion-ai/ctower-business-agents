import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { FormRepositorios } from "./FormRepositorios";
import { ProntidaoRepositorios } from "./ProntidaoRepositorios";
import { SEM_REPOSITORIOS, validarRepositorios, type Repositorios } from "./repositorios";
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

function Controlado({
  inicial = SEM_REPOSITORIOS,
  aoMudar,
}: {
  inicial?: Repositorios;
  aoMudar?: (r: Repositorios) => void;
}) {
  const [v, setV] = useState(inicial);
  return (
    <FormRepositorios
      valor={v}
      aoMudar={(r) => {
        setV(r);
        aoMudar?.(r);
      }}
    />
  );
}

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
    expect(validarRepositorios({ forge: gitlab, tracker: JIRA })).toBeNull();
    expect(
      validarRepositorios({
        forge: { kind: "github", repo: "o/r" },
        tracker: gitlab,
      }),
    ).toBeNull();
  });

  it("recusa com as regras do Jira do ctower-agents", () => {
    const t = (p: object) =>
      validarRepositorios({ forge: null, tracker: { ...JIRA, ...p } as never });
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
    fireEvent.change(screen.getByLabelText(/Repositório do código-fonte/), {
      target: { value: "gitlab" },
    });
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
    const ultimo = aoMudar.mock.calls[aoMudar.mock.calls.length - 1]![0] as Repositorios;
    expect(ultimo.forge?.kind).toBe("gitlab");
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
    fireEvent.change(grupo.getByLabelText(/Repositório do código-fonte/), {
      target: { value: "github" },
    });
    fireEvent.change(grupo.getByLabelText(/Repositório do código \(dono/), {
      target: { value: "o/r" },
    });
    fireEvent.click(grupo.getByRole("button", { name: "Salvar repositórios" }));
    await waitFor(() =>
      expect(api.salvarRepositorios).toHaveBeenCalledWith("faturamento-exemplo", {
        forge: { kind: "github", repo: "o/r" },
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

  it("com o tipo já gravado, o seletor fica travado", async () => {
    const cfg = configuracoesFicticias();
    cfg.projetos[0].repositorios = {
      forge: { kind: "github", repo: "o/r" },
      tracker: JIRA,
    };
    render(<TelaConfiguracoes api={apiConfiguracoesFalsa(cfg)} aoMudarProjetos={vi.fn()} />);
    const grupo = within(
      await screen.findByRole("group", {
        name: "Repositórios de Faturamento Exemplo",
      }),
    );
    expect(grupo.getByLabelText(/Repositório do código-fonte/)).toBeDisabled();
    expect(grupo.getByLabelText(/Repositório de demandas/)).toBeDisabled();
  });
});
