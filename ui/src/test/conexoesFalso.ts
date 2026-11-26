import { vi } from "vitest";
import type {
  ApiConexoes,
  Conexao,
  ConexaoVista,
  Descritor,
  UsoConexao,
} from "../conexoes";
import type { IntegracaoProntidao } from "../repositorios";

/** Os descritores como o backend os devolve (subconjunto do `ResumoDescritor`). */
export function descritoresFicticios(): Descritor[] {
  const fontes = (lista: [string, string][]) =>
    lista.map(([fonte, rotulo]) => ({ fonte, rotulo })) as Descritor["fontes_aceitas"];
  return [
    {
      tipo: "github",
      rotulo: "GitHub",
      icone: "github",
      capacidades: ["forja", "rastreador"],
      endereco_padrao: "github.com",
      formato_endereco: "host, como github.com",
      fontes_aceitas: fontes([
        ["chaveiro", "Chaveiro do sistema"],
        ["cli", "Login da própria CLI"],
        ["comando", "Comando que imprime a credencial"],
      ]),
      fonte_padrao: "chaveiro",
      campos_opcoes: [],
      passos_prontidao: [],
    },
    {
      tipo: "gitlab",
      rotulo: "GitLab",
      icone: "gitlab",
      capacidades: ["forja", "rastreador"],
      endereco_padrao: "gitlab.com",
      formato_endereco: "host, como gitlab.example.com",
      fontes_aceitas: fontes([
        ["chaveiro", "Chaveiro do sistema"],
        ["comando", "Comando que imprime a credencial"],
      ]),
      fonte_padrao: "chaveiro",
      campos_opcoes: [],
      passos_prontidao: [],
    },
    {
      tipo: "jira",
      rotulo: "Jira",
      icone: "jira",
      capacidades: ["rastreador"],
      endereco_padrao: "",
      formato_endereco: "URL do servidor, como https://jira.example.com",
      fontes_aceitas: fontes([
        ["chaveiro", "Chaveiro do sistema"],
        ["comando", "Comando que imprime a credencial"],
      ]),
      fonte_padrao: "chaveiro",
      campos_opcoes: [
        {
          chave: "config_file",
          rotulo: "Arquivo de configuração do jira-cli",
          ajuda: "Vazio usa ~/.config/.jira/.config.yml.",
          tipo: "texto",
          padrao: null,
          obrigatorio: false,
        },
      ],
      passos_prontidao: [],
    },
  ];
}

export const GITLAB_INTERNO: Conexao = {
  id: "gitlab-interno",
  nome: "GitLab interno",
  tipo: "gitlab",
  endereco: "gitlab.example.com",
  credencial: { fonte: "chaveiro", conta: "conexao:gitlab-interno" },
};

export const JIRA_EMPRESA: Conexao = {
  id: "jira-da-empresa",
  nome: "Jira da empresa",
  tipo: "jira",
  endereco: "https://jira.example.com",
  credencial: { fonte: "comando", cmd: "pass show jira | head -1" },
  opcoes: { config_file: "/home/analista/.config/.jira/.config.yml" },
};

const USOS: Record<string, UsoConexao[]> = {
  "gitlab-interno": [
    { projeto: "faturamento-exemplo", nomeProjeto: "Faturamento Exemplo", onde: "código g/fat" },
    { projeto: "cobranca-exemplo", nomeProjeto: "Cobrança Exemplo", onde: "código g/cob" },
  ],
};

/** API das conexões falsa, com um GitLab em uso por dois projetos e um Jira livre. */
export function apiConexoesFalsa(iniciais: Conexao[] = [GITLAB_INTERNO, JIRA_EMPRESA]) {
  const conexoes = [...iniciais];
  const guardadas = new Map<string, string>([["gitlab-interno", "glpat-ficticio"]]);
  const testes = new Map<string, ConexaoVista["ultimoTeste"]>();
  const descritores = descritoresFicticios();
  const vista = (c: Conexao): ConexaoVista => {
    const d = descritores.find((x) => x.tipo === c.tipo)!;
    return {
      conexao: c,
      rotuloTipo: d.rotulo,
      icone: d.icone,
      credencialGuardada: c.credencial.fonte === "chaveiro" ? guardadas.has(c.id) : null,
      usos: USOS[c.id] ?? [],
      ultimoTeste: testes.get(c.id) ?? null,
    };
  };
  let proximoTeste: (id: string) => IntegracaoProntidao = (id) => ({
    titulo: id,
    passos: [
      { id: "cli", rotulo: "GitLab CLI (glab) instalada", situacao: "ok", detalhe: "glab 1.48.0 (/usr/bin/glab)", orientacao: null },
      { id: "credencial", rotulo: "Credencial disponível", situacao: "ok", detalhe: "chaveiro do sistema", orientacao: null },
      {
        id: "versao_servidor",
        rotulo: "Acesso ao GitLab (versão do servidor)",
        situacao: "falhou",
        detalhe: "401 Unauthorized",
        orientacao: "Confira o host e se o token vale para ele.",
      },
    ],
  });
  const api = {
    descritoresConexao: vi.fn(() => Promise.resolve(descritores)),
    listarConexoes: vi.fn(() => Promise.resolve(conexoes.map(vista))),
    salvarConexao: vi.fn((c: Conexao) => {
      const id = c.id || c.nome.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      const gravada: Conexao = {
        ...c,
        id,
        credencial:
          c.credencial.fonte === "chaveiro" && !c.credencial.conta
            ? { fonte: "chaveiro", conta: `conexao:${id}` }
            : c.credencial,
      };
      const i = conexoes.findIndex((x) => x.id === id);
      if (i >= 0) conexoes[i] = gravada;
      else conexoes.push(gravada);
      return Promise.resolve(gravada);
    }),
    removerConexao: vi.fn((id: string) => {
      const usos = USOS[id] ?? [];
      if (usos.length > 0) {
        const c = conexoes.find((x) => x.id === id)!;
        return Promise.reject(
          new Error(
            `a conexão "${c.nome}" não pode ser removida: ela é usada por ${usos
              .map((u) => u.nomeProjeto)
              .join(", ")}. Troque a conexão nesses projetos antes`,
          ),
        );
      }
      conexoes.splice(
        conexoes.findIndex((x) => x.id === id),
        1,
      );
      return Promise.resolve();
    }),
    guardarCredencialConexao: vi.fn((id: string, valor: string) => {
      guardadas.set(id, valor);
      return Promise.resolve();
    }),
    testarConexao: vi.fn((id: string) => {
      const r = proximoTeste(id);
      const falha = r.passos.find((p) => p.situacao === "falhou");
      testes.set(id, {
        pronta: !falha,
        quandoMs: 1_790_000_000_000,
        resumo: falha ? `${falha.rotulo}: ${falha.detalhe}` : "pronta",
      });
      return Promise.resolve(r);
    }),
  } satisfies ApiConexoes;
  return Object.assign(api, {
    conexoes,
    guardadas,
    aoTestar: (f: (id: string) => IntegracaoProntidao) => {
      proximoTeste = f;
    },
  });
}
