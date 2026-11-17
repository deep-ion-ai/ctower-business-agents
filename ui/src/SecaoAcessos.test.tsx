import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  dataCurta,
  descreverUsos,
  haQuanto,
  hostsDosProjetos,
  repositoriosComTokenProprio,
  situacaoDoToken,
} from "./acessos";
import type { Configuracoes } from "./configuracoes";
import { REPOSITORIO_CODIGO_VAZIO, type RepositorioCodigo } from "./repositorios";
import { SecaoAcessos } from "./SecaoAcessos";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { apiConfiguracoesFalsa, configuracoesFicticias } from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

/** 16/11/2026, meio-dia (horário local). */
const AGORA = new Date(2026, 10, 16, 12).getTime();
const DIA = 24 * 60 * 60 * 1000;

const API_GITLAB: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  forja: "gitlab",
  host: "gitlab.empresa.com.br",
  caminho: "fin/faturamento-api",
  url_clone: "https://gitlab.empresa.com.br/fin/faturamento-api.git",
};

const WEB_PROPRIO: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  caminho: "empresa/faturamento-web",
  url_clone: "https://github.com/empresa/faturamento-web.git",
  token: "proprio",
};

const PORTAL: RepositorioCodigo = {
  ...REPOSITORIO_CODIGO_VAZIO,
  caminho: "empresa/portal",
  url_clone: "https://github.com/empresa/portal.git",
};

function cenario(): Configuracoes {
  const cfg = configuracoesFicticias();
  cfg.projetos = [
    {
      ...cfg.projetos[0]!,
      id: "faturamento",
      nome: "Faturamento",
      repositorios: { codigo: [API_GITLAB, WEB_PROPRIO], tracker: null },
    },
    {
      ...cfg.projetos[0]!,
      id: "portal",
      nome: "Portal do Cliente",
      repositorios: {
        codigo: [PORTAL],
        tracker: { kind: "github", repo: "empresa/portal" },
      },
    },
  ];
  return cfg;
}

function montar(cfg = cenario(), aoAbrirProjeto = vi.fn()) {
  const api = apiConfiguracoesFalsa(cfg);
  render(<SecaoAcessos api={api} cfg={cfg} aoAbrirProjeto={aoAbrirProjeto} agora={() => AGORA} />);
  return { api, aoAbrirProjeto };
}

const hosts = () => within(screen.getByRole("region", { name: "Token global por host" }));
const linha = (nome: string) => within(screen.getByRole("listitem", { name: nome }));

describe("quem usa cada token", () => {
  it("lista os hosts com os projetos que usam o token global", () => {
    const h = hostsDosProjetos(cenario(), [{ host: "Git.Example.org", forja: "gitlab" }]);
    expect(h.map((x) => [x.host, x.forja, x.extra])).toEqual([
      ["github.com", "github", false],
      ["gitlab.empresa.com.br", "gitlab", false],
      ["git.example.org", "gitlab", true],
    ]);
    // O repositório com token próprio não usa o do host.
    expect(descreverUsos(h[0]!.usos)).toBe("Portal do Cliente (portal, issues)");
    expect(descreverUsos(h[1]!.usos)).toBe("Faturamento (faturamento-api)");
    expect(h[1]!.exemplo?.caminho).toBe("fin/faturamento-api");
    expect(repositoriosComTokenProprio(cenario()).map((r) => r.repositorio.caminho)).toEqual([
      "empresa/faturamento-web",
    ]);
  });

  it("descreve a situação: sem token, não testado, ok, vencendo, expirado e falha", () => {
    const s = (e: Parameters<typeof situacaoDoToken>[0]) => situacaoDoToken(e, AGORA);
    expect(s({ guardado: false, comando: null, teste: null }).texto).toBe("sem token");
    expect(s({ guardado: true, comando: null, teste: null }).texto).toBe("não testado");
    expect(s({ guardado: false, comando: "pass x", teste: null }).texto).toBe("não testado");
    const ok = s({
      guardado: true,
      comando: null,
      teste: {
        ok: true,
        quando: AGORA - 2 * DIA,
        usuario: "ana",
        expira: "2027-03-01",
      },
    });
    expect(ok).toEqual({
      tom: "ok",
      texto: "✓ acesso ok · há 2 dias",
      detalhe: "token de ana · válido até 01/03/2027",
    });
    expect(
      s({
        guardado: true,
        comando: null,
        teste: { ok: true, quando: AGORA, expira: "2026-11-20" },
      }),
    ).toMatchObject({ tom: "atencao", texto: "! vence em 20/11" });
    expect(
      s({
        guardado: true,
        comando: null,
        teste: {
          ok: false,
          quando: AGORA,
          mensagem: "Bad credentials",
          expira: "2026-11-10",
        },
      }),
    ).toMatchObject({ tom: "erro", texto: "✕ expirou em 10/11" });
    expect(
      s({
        guardado: true,
        comando: null,
        teste: { ok: false, quando: AGORA - DIA, mensagem: "401" },
      }),
    ).toEqual({ tom: "erro", texto: "✕ sem acesso · ontem", detalhe: "401" });
    expect(haQuanto(AGORA, AGORA)).toBe("hoje");
    expect(dataCurta("2026-11-10", AGORA)).toBe("10/11");
  });
});

describe("seção Acessos", () => {
  it("mostra host, forja, quem usa e o estado do último teste, sem o token", async () => {
    const cfg = cenario();
    const api = apiConfiguracoesFalsa(cfg);
    api.chaveiro.set("github.com", "ghp-segredo");
    api.testes.set("github.com", {
      ok: false,
      quando: AGORA,
      mensagem: "Bad credentials (HTTP 401)",
      expira: "2026-11-10",
    });
    const { container } = render(<SecaoAcessos api={api} cfg={cfg} agora={() => AGORA} />);
    const gh = within(await screen.findByRole("listitem", { name: "github.com" }));
    expect(gh.getByText("GitHub")).toBeInTheDocument();
    expect(gh.getByText("Usado por Portal do Cliente (portal, issues)")).toBeInTheDocument();
    expect(await gh.findByText("✕ expirou em 10/11")).toHaveAttribute("data-tom", "erro");
    expect(gh.getByText("Bad credentials (HTTP 401)")).toBeInTheDocument();
    const gl = linha("gitlab.empresa.com.br");
    expect(gl.getByText("Usado por Faturamento (faturamento-api)")).toBeInTheDocument();
    expect(gl.getByText("sem token")).toBeInTheDocument();
    expect(
      gl.getByRole("button", {
        name: "Testar o token de gitlab.empresa.com.br",
      }),
    ).toBeDisabled();
    expect(container.innerHTML).not.toContain("ghp-segredo");
  });

  it("trocar o token do host vale para todos que o usam e limpa o campo", async () => {
    const { api } = montar();
    api.chaveiro.set("github.com", "ghp-velho");
    const gh = within(await screen.findByRole("listitem", { name: "github.com" }));
    fireEvent.click(await gh.findByRole("button", { name: "Trocar o token de github.com" }));
    const campo = gh.getByLabelText("Novo token de github.com");
    expect(campo).toHaveAttribute("type", "password");
    fireEvent.change(campo, { target: { value: "ghp-novo" } });
    fireEvent.click(gh.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.salvarToken).toHaveBeenCalledWith(
        { host: "github.com", caminho: null },
        "ghp-novo",
      ),
    );
    expect(
      await gh.findByText("Token guardado. Vale agora para Portal do Cliente (portal, issues)."),
    ).toBeInTheDocument();
    expect(api.chaveiro.get("github.com")).toBe("ghp-novo");
    expect(gh.queryByLabelText("Novo token de github.com")).not.toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain("ghp-novo");
  });

  it("testa o token do host e mostra a validade", async () => {
    const { api } = montar();
    api.chaveiro.set("gitlab.empresa.com.br", "glpat");
    api.aoTestar(() => ({
      ok: true,
      quando: AGORA,
      usuario: "bia",
      expira: "2027-01-31",
    }));
    const gl = within(await screen.findByRole("listitem", { name: "gitlab.empresa.com.br" }));
    const testar = gl.getByRole("button", {
      name: "Testar o token de gitlab.empresa.com.br",
    });
    await waitFor(() => expect(testar).toBeEnabled());
    fireEvent.click(testar);
    expect(await gl.findByText("✓ acesso ok · hoje")).toBeInTheDocument();
    expect(gl.getByText("token de bia · válido até 31/01/2027")).toBeInTheDocument();
    expect(api.testarTokenHost).toHaveBeenCalledWith("gitlab.empresa.com.br", "gitlab", API_GITLAB);
  });

  it("remover pede confirmação nomeando quem perde o acesso", async () => {
    const { api } = montar();
    api.chaveiro.set("github.com", "ghp");
    const gh = within(await screen.findByRole("listitem", { name: "github.com" }));
    const remover = gh.getByRole("button", {
      name: "Remover o token de github.com",
    });
    await waitFor(() => expect(remover).toBeEnabled());
    fireEvent.click(remover);
    const dialogo = within(screen.getByRole("dialog", { name: "Remover o token de github.com?" }));
    expect(dialogo.getByText("Portal do Cliente (portal, issues)")).toBeInTheDocument();
    fireEvent.click(dialogo.getByRole("button", { name: "Cancelar" }));
    expect(api.removerHostAcesso).not.toHaveBeenCalled();
    fireEvent.click(remover);
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Remover",
      }),
    );
    await waitFor(() => expect(api.removerHostAcesso).toHaveBeenCalledWith("github.com"));
    expect(api.chaveiro.has("github.com")).toBe(false);
    expect(await gh.findByText("sem token")).toBeInTheDocument();
  });

  it("usa um comando que imprime o token no lugar do token do host", async () => {
    const { api } = montar();
    const gl = within(await screen.findByRole("listitem", { name: "gitlab.empresa.com.br" }));
    fireEvent.click(
      gl.getByRole("button", {
        name: "Informar o token de gitlab.empresa.com.br",
      }),
    );
    fireEvent.click(
      gl.getByLabelText("Comando que imprime o token", {
        selector: "input[type=radio]",
      }),
    );
    fireEvent.change(
      gl.getByLabelText("Comando que imprime o token", {
        selector: "input[type=text]",
      }),
      {
        target: { value: "pass show gitlab | head -1" },
      },
    );
    fireEvent.click(gl.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.definirComandoHost).toHaveBeenCalledWith(
        "gitlab.empresa.com.br",
        "pass show gitlab | head -1",
      ),
    );
    expect(await gl.findByText("pass show gitlab | head -1")).toBeInTheDocument();
    expect(gl.getByText("não testado")).toBeInTheDocument();
  });

  it("adiciona um host com o token", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "+ Adicionar host" }));
    const novo = within(screen.getByRole("group", { name: "Adicionar host" }));
    fireEvent.change(novo.getByLabelText("Host"), {
      target: { value: "Git.Example.org" },
    });
    fireEvent.change(novo.getByLabelText("Token (PAT)"), {
      target: { value: "glpat-x" },
    });
    fireEvent.click(novo.getByRole("button", { name: "Adicionar" }));
    await waitFor(() =>
      expect(api.adicionarHostAcesso).toHaveBeenCalledWith("git.example.org", "gitlab"),
    );
    expect(api.salvarToken).toHaveBeenCalledWith(
      { host: "git.example.org", caminho: null },
      "glpat-x",
    );
    const linhaNova = within(await hosts().findByRole("listitem", { name: "git.example.org" }));
    expect(linhaNova.getByText("Nenhum projeto usa este host ainda")).toBeInTheDocument();
    expect(await linhaNova.findByText("não testado")).toBeInTheDocument();
  });

  it("lista os repositórios com token próprio, com precedência e atalho para o projeto", async () => {
    const { api, aoAbrirProjeto } = montar();
    const r = within(
      await screen.findByRole("listitem", {
        name: "github.com/empresa/faturamento-web",
      }),
    );
    expect(r.getByText(/tem precedência sobre o token do host/)).toBeInTheDocument();
    expect(await r.findByText("✕ sem token no chaveiro")).toBeInTheDocument();
    fireEvent.click(r.getByRole("button", { name: "Abrir o projeto Faturamento" }));
    expect(aoAbrirProjeto).toHaveBeenCalledWith("faturamento");
    fireEvent.click(
      r.getByRole("button", {
        name: "Testar o token de github.com/empresa/faturamento-web",
      }),
    );
    await waitFor(() => expect(api.testarTokenRepositorio).toHaveBeenCalledWith(WEB_PROPRIO));
  });

  it("aparece no item Acessos do menu das configurações", async () => {
    localStorage.clear();
    const api = apiConfiguracoesFalsa(cenario());
    render(<TelaConfiguracoes api={api} aoMudarProjetos={vi.fn()} />);
    const menu = within(await screen.findByRole("navigation", { name: /configura/i }));
    fireEvent.click(menu.getByRole("button", { name: "Acessos" }));
    expect(await screen.findByRole("region", { name: "Acessos" })).toBeInTheDocument();
    expect(
      await screen.findByRole("listitem", { name: "gitlab.empresa.com.br" }),
    ).toBeInTheDocument();
    localStorage.clear();
  });
});
