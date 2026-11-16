import type { SituacaoProtecao } from "../analise";
import {
  CONFLITO,
  type AjusteLeitura,
  type AmbienteDesfeito,
  type AmbienteDetalhado,
  type ApiConfiguracoes,
  type AvaliacaoTroca,
  type AvisoTexto,
  type Configuracoes,
  type EstadoTexto,
  type InfoBase,
  type LeituraTexto,
  type ModoTroca,
  type SalvoTexto,
  type TextoEditavel,
  type TrocaBase,
} from "../configuracoes";
import type { PreviaProjeto } from "../primeiroUso";
import type { EstadoToken, HostCadastrado, TesteToken } from "../acessos";
import type {
  AmbienteTeste,
  EntradaAmbienteTeste,
  SituacaoAmbienteTeste,
} from "../ambienteTeste";
import type {
  AlvoToken,
  ForgeKind,
  EventoClone,
  Prontidao,
  RepoDetectado,
  RepositorioCodigo,
  Repositorios,
  ResultadoClone,
} from "../repositorios";
import { apiPrimeiroUsoFalsa, inventarioFicticio } from "./falsos";

export const BASE = "/Users/analista/Documents/ctower-business";
export const CODIGO = "/codigo/faturamento-exemplo";
export const PASTA_CLONES = "/Users/analista/Documents/ctower-business-repos";

/** Conta do chaveiro falso: `host` (global) ou `host/caminho` (próprio). */
export function contaDoAlvo(alvo: AlvoToken): string {
  const caminho = alvo.caminho?.trim();
  return caminho ? `${alvo.host}/${caminho}` : alvo.host;
}

export function configuracoesFicticias(parcial: Partial<Configuracoes> = {}): Configuracoes {
  return {
    cliente: "claudeDesktop",
    base: BASE,
    baseSugerida: BASE,
    projetos: [
      {
        id: "faturamento-exemplo",
        nome: "Faturamento Exemplo",
        pastas: [{ caminho: CODIGO, prefixo: null, existe: true }],
        exclusoes: ["docs/antigos/"],
        inclusoes: [],
        repositorios: { codigo: [], tracker: null },
      },
    ],
    configClaude: {
      estado: "/Users/analista/.claude.json",
      settings: "/Users/analista/.claude/settings.json",
    },
    ...parcial,
  };
}

export const TEXTOS: TextoEditavel[] = [
  {
    grupo: "template",
    id: "analise-impacto",
    rotulo: "Análise de impacto",
    arquivo: "_templates/analise-impacto.md",
    estado: "editado",
  },
  {
    grupo: "roteiro",
    id: "perguntar",
    rotulo: "Perguntar ao sistema",
    arquivo: "_templates/roteiros/perguntar.md",
    estado: "padrao",
  },
  {
    grupo: "prompt",
    id: "aprofundar",
    rotulo: "Aprofundar",
    arquivo: "_templates/prompts/aprofundar.md",
    estado: "igualAoPadrao",
  },
  {
    grupo: "regrasDoProjeto",
    id: "faturamento-exemplo",
    rotulo: "Regras do projeto Faturamento Exemplo",
    arquivo: "faturamento-exemplo/regras-do-projeto.md",
    estado: "igualAoPadrao",
  },
];

/** Texto padrão de cada arquivo do catálogo falso. */
export function padraoFalso(t: TextoEditavel): string {
  return `<!--\nInstrução: use {{id}} e {{titulo}}.\n-->\n# {{id}} · ${t.rotulo}\n\n## Corpo\n\n{{corpo}}\n`;
}

const VARIAVEIS_FALSAS = [
  { nome: "id", descricao: "Identificador." },
  { nome: "titulo", descricao: "Título." },
  { nome: "corpo", descricao: "Corpo do documento." },
];

/** Os avisos que o app daria: marcador que o texto não aceita (só fora das regras). */
function avisosFalsos(t: TextoEditavel, texto: string): AvisoTexto[] {
  if (t.grupo === "regrasDoProjeto") return [];
  const avisos: AvisoTexto[] = [];
  texto.split("\n").forEach((linha, i) => {
    for (const m of linha.matchAll(/\{\{([a-z0-9_]+)\}\}/g)) {
      if (!VARIAVEIS_FALSAS.some((v) => v.nome === m[1])) {
        avisos.push({
          linha: i + 1,
          variavel: m[1],
          ausente: false,
          mensagem: `marcador desconhecido {{${m[1]}}}`,
        });
      }
    }
  });
  return avisos;
}

function protecao(ligada: boolean): SituacaoProtecao {
  return {
    projeto: "faturamento-exemplo",
    ligada,
    completa: ligada,
    arquivo: "/Users/analista/.claude/settings.json",
    regras: [{ regra: `Edit(/${CODIGO}/**)`, presente: ligada, doApp: ligada }],
    negadasPeloHook: ["Bash", "PowerShell"],
  };
}

/** API das configurações falsa: tudo dá certo, salvo o que o teste mudar. */
export function apiConfiguracoesFalsa(cfg: Configuracoes = configuracoesFicticias()) {
  const base = apiPrimeiroUsoFalsa();
  // Os arquivos da base: `texto` nulo é "sem cópia" (vale o padrão do app).
  let contador = 0;
  const arquivos = new Map(
    TEXTOS.map((t) => [
      t.arquivo,
      {
        texto:
          t.estado === "padrao"
            ? null
            : t.estado === "editado"
              ? `${padraoFalso(t)}\nAcréscimo do analista.\n`
              : padraoFalso(t),
        versao: `v${++contador}`,
      },
    ]),
  );
  const ambientes = new Map<string, AmbienteTeste>(
    cfg.projetos.flatMap((p) => (p.ambienteTeste ? [[p.id, p.ambienteTeste] as const] : [])),
  );
  const senhasTeste = new Map<string, string>();
  const situacaoTeste = (projeto: string): SituacaoAmbienteTeste => {
    const ambiente = ambientes.get(projeto) ?? null;
    return {
      ambiente,
      comSenha: (ambiente?.usuarios ?? [])
        .map((u) => u.id)
        .filter((id) => senhasTeste.has(`${projeto}/${id}`)),
    };
  };
  const catalogo = (arquivo: string) => {
    const t = TEXTOS.find((x) => x.arquivo === arquivo);
    if (!t) throw new Error(`${arquivo} não é um texto editável da base`);
    return t;
  };
  const estadoDe = (t: TextoEditavel): EstadoTexto => {
    const a = arquivos.get(t.arquivo)!;
    if (a.texto === null) return "padrao";
    return a.texto === padraoFalso(t) ? "igualAoPadrao" : "editado";
  };
  const versaoDe = (arquivo: string) =>
    arquivos.get(arquivo)!.texto === null ? "ausente" : arquivos.get(arquivo)!.versao;
  const leitura = (arquivo: string): LeituraTexto => {
    const t = catalogo(arquivo);
    return {
      grupo: t.grupo,
      id: t.id,
      rotulo: t.rotulo,
      arquivo,
      texto: arquivos.get(arquivo)!.texto ?? padraoFalso(t),
      padrao: padraoFalso(t),
      estado: estadoDe(t),
      variaveis: t.grupo === "regrasDoProjeto" ? [] : VARIAVEIS_FALSAS,
      versao: versaoDe(arquivo),
    };
  };
  /** O chaveiro falso: conta → token. */
  const chaveiro = new Map<string, string>();
  /** Comandos que imprimem o token global (host → comando). */
  const comandos = new Map<string, string>();
  /** Últimos testes (conta → resultado). */
  const testes = new Map<string, TesteToken>();
  let hostsExtras: HostCadastrado[] = [];
  /** Resultado do próximo "Testar" (o padrão: acesso ok agora). */
  let proximoTeste: () => TesteToken = () => ({ ok: true, quando: Date.now() });
  let pastaClones: string | null = PASTA_CLONES;
  const api = {
    ...base,
    pastaClones: vi.fn((): Promise<string | null> => Promise.resolve(pastaClones)),
    definirPastaClones: vi.fn((pasta: string): Promise<string> => {
      if (!pasta.startsWith("/")) {
        return Promise.reject(`informe o caminho completo da pasta dos clones: ${pasta}`);
      }
      pastaClones = pasta;
      return Promise.resolve(pasta);
    }),
    salvarToken: vi.fn((alvo: AlvoToken, token: string): Promise<void> => {
      chaveiro.set(contaDoAlvo(alvo), token);
      if (!alvo.caminho) comandos.delete(alvo.host);
      testes.delete(contaDoAlvo(alvo));
      return Promise.resolve();
    }),
    apagarToken: vi.fn((alvo: AlvoToken): Promise<void> => {
      chaveiro.delete(contaDoAlvo(alvo));
      testes.delete(contaDoAlvo(alvo));
      return Promise.resolve();
    }),
    hostsAcessos: vi.fn((): Promise<HostCadastrado[]> => Promise.resolve([...hostsExtras])),
    adicionarHostAcesso: vi.fn((host: string, forja: ForgeKind): Promise<HostCadastrado> => {
      const h = { host: host.trim().toLowerCase(), forja };
      hostsExtras = [...hostsExtras.filter((x) => x.host !== h.host), h];
      return Promise.resolve(h);
    }),
    removerHostAcesso: vi.fn((host: string): Promise<void> => {
      chaveiro.delete(host);
      comandos.delete(host);
      testes.delete(host);
      hostsExtras = hostsExtras.filter((x) => x.host !== host);
      return Promise.resolve();
    }),
    definirComandoHost: vi.fn((host: string, comando: string): Promise<void> => {
      if (!comando.trim()) return Promise.reject("informe o comando que imprime o token");
      comandos.set(host, comando.trim());
      chaveiro.delete(host);
      testes.delete(host);
      return Promise.resolve();
    }),
    estadoTokens: vi.fn(
      (alvos: AlvoToken[]): Promise<EstadoToken[]> =>
        Promise.resolve(
          alvos.map((a) => ({
            guardado: chaveiro.has(contaDoAlvo(a)),
            comando: a.caminho ? null : (comandos.get(a.host) ?? null),
            teste: testes.get(contaDoAlvo(a)) ?? null,
          })),
        ),
    ),
    testarTokenHost: vi.fn(
      (
        host: string,
        _forja: ForgeKind,
        _exemplo: RepositorioCodigo | null,
      ): Promise<TesteToken> => {
        const t = proximoTeste();
        testes.set(host, t);
        return Promise.resolve(t);
      },
    ),
    testarTokenRepositorio: vi.fn((r: RepositorioCodigo): Promise<TesteToken> => {
      const t = proximoTeste();
      testes.set(contaDoAlvo({ host: r.host, caminho: r.caminho }), t);
      return Promise.resolve(t);
    }),
    tokensGuardados: vi.fn(
      (alvos: AlvoToken[]): Promise<boolean[]> =>
        Promise.resolve(alvos.map((a) => chaveiro.has(contaDoAlvo(a)))),
    ),
    testarAcesso: vi.fn(
      (_repositorio: RepositorioCodigo, _token?: string | null): Promise<void> =>
        Promise.resolve(),
    ),
    clonarRepositorios: vi.fn(
      (
        repositorios: RepositorioCodigo[],
        pastaBase: string,
        aoEvento: (e: EventoClone) => void,
      ): Promise<ResultadoClone[]> =>
        Promise.resolve(
          repositorios.map((r, indice) => {
            const pasta = `${pastaBase}/${r.caminho.split("/").pop()}`;
            aoEvento({ tipo: "inicio", indice });
            aoEvento({ tipo: "concluido", indice, pasta });
            return { repositorio: { ...r, pasta }, erro: null };
          }),
        ),
    ),
    // Ambiente de teste: o cadastro e o "chaveiro" das senhas, como o app faz.
    ambienteTeste: vi.fn(
      (projeto: string): Promise<SituacaoAmbienteTeste> =>
        Promise.resolve(situacaoTeste(projeto)),
    ),
    salvarAmbienteTeste: vi.fn(
      (projeto: string, e: EntradaAmbienteTeste): Promise<SituacaoAmbienteTeste> => {
        const anterior = ambientes.get(projeto)?.usuarios ?? [];
        let maior = Math.max(0, ...anterior.map((u) => Number(u.id.slice(1)) || 0));
        const usuarios = e.usuarios.map((u) => {
          const id = u.id && anterior.some((a) => a.id === u.id) ? u.id : `u${++maior}`;
          if (u.senha) senhasTeste.set(`${projeto}/${id}`, u.senha);
          return {
            id,
            usuario: u.usuario.trim(),
            perfil: u.perfil.trim(),
            descricao: u.descricao.trim(),
          };
        });
        for (const a of anterior)
          if (!usuarios.some((u) => u.id === a.id)) senhasTeste.delete(`${projeto}/${a.id}`);
        ambientes.set(projeto, {
          url: e.url.trim(),
          tipo: "web",
          autenticacao: "usuario_senha",
          observacoes: e.observacoes.trim(),
          usuarios,
        });
        return Promise.resolve(situacaoTeste(projeto));
      },
    ),
    removerAmbienteTeste: vi.fn((projeto: string): Promise<SituacaoAmbienteTeste> => {
      for (const u of ambientes.get(projeto)?.usuarios ?? [])
        senhasTeste.delete(`${projeto}/${u.id}`);
      ambientes.delete(projeto);
      return Promise.resolve(situacaoTeste(projeto));
    }),
    configuracoes: vi.fn((): Promise<Configuracoes> => Promise.resolve(cfg)),
    infoBase: vi.fn(
      (): Promise<InfoBase | null> =>
        Promise.resolve(
          cfg.base
            ? {
                caminho: cfg.base,
                existe: true,
                arquivos: 142,
                bytes: 3_460_000,
                git: true,
                indiceAtualizadoEm: 1_700_000_000,
              }
            : null,
        ),
    ),
    abrirBase: vi.fn((): Promise<string> => Promise.resolve(cfg.base ?? "")),
    salvarRepositorios: vi.fn(
      (_projeto: string, repositorios: Repositorios): Promise<Repositorios> =>
        Promise.resolve(repositorios),
    ),
    prontidaoRepositorios: vi.fn((): Promise<Prontidao> => Promise.resolve({ integracoes: [] })),
    detectarRepositorios: vi.fn((): Promise<RepoDetectado[]> => Promise.resolve([])),
    verificarAmbiente: vi.fn(
      (): Promise<AmbienteDetalhado> =>
        Promise.resolve({
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
        }),
    ),
    avaliarTroca: vi.fn(
      (destino: string): Promise<AvaliacaoTroca> =>
        Promise.resolve({
          destino: { caminho: destino, estado: "nova", recusa: null, git: false },
          atual: cfg.base,
          arquivos: 12,
          bytes: 40_960,
          recusaMover: null,
          recusaReapontar: null,
          sessoesAbertas: 0,
        }),
    ),
    trocarBase: vi.fn(
      (destino: string, modo: ModoTroca): Promise<TrocaBase> =>
        Promise.resolve({
          modo,
          base: destino,
          anterior: cfg.base,
          copiados: modo === "mover" ? 12 : 0,
          origemApagada: modo === "mover",
          indexados: 5,
          avisos: [],
        }),
    ),
    previaLeitura: vi.fn(
      (a: AjusteLeitura): Promise<PreviaProjeto> =>
        Promise.resolve({
          id: a.projeto,
          nome: "Faturamento Exemplo",
          pastas: a.pastas.map((caminho, i) => ({
            caminho,
            prefixo: i === 0 ? null : (caminho.split("/").pop() ?? null),
            branch: null,
            git: false,
          })),
          exclusoes: a.exclusoes,
          inclusoes: a.inclusoes,
          jaCadastrado: false,
          inventario: inventarioFicticio(),
        }),
    ),
    salvarLeitura: vi.fn((a: AjusteLeitura) =>
      Promise.resolve({
        id: a.projeto,
        nome: "Faturamento Exemplo",
        pastas: a.pastas.map((caminho, i) => ({
          caminho,
          prefixo: i === 0 ? null : (caminho.split("/").pop() ?? null),
          branch: null,
          git: false,
        })),
        inventario: inventarioFicticio(),
      }),
    ),
    desfazerAmbiente: vi.fn(
      (_s: string): Promise<AmbienteDesfeito> =>
        Promise.resolve({
          protecaoTirada: [],
          preparo: { diretorio: CODIGO, alteracoes: [], gravado: true },
          avisos: [],
          igualAoAntes: true,
        }),
    ),
    descadastrar: vi.fn((_s: string) =>
      Promise.resolve({
        ambiente: {
          protecaoTirada: [],
          preparo: { diretorio: CODIGO, alteracoes: [], gravado: true },
          avisos: [],
          igualAoAntes: true,
        },
        documentos: `${BASE}/faturamento-exemplo`,
      }),
    ),
    textos: vi.fn(
      (): Promise<TextoEditavel[]> =>
        Promise.resolve(TEXTOS.map((t) => ({ ...t, estado: estadoDe(t) }))),
    ),
    abrirTexto: vi.fn((arquivo: string) => {
      const t = catalogo(arquivo);
      const a = arquivos.get(arquivo)!;
      // Como o app: abrir um texto sem cópia cria a cópia do padrão.
      if (a.texto === null) arquivos.set(arquivo, { texto: padraoFalso(t), versao: `v${++contador}` });
      return Promise.resolve(`${BASE}/${arquivo}`);
    }),
    lerTexto: vi.fn((arquivo: string): Promise<LeituraTexto> => Promise.resolve(leitura(arquivo))),
    validarTexto: vi.fn(
      (arquivo: string, texto: string): Promise<AvisoTexto[]> =>
        Promise.resolve(avisosFalsos(catalogo(arquivo), texto)),
    ),
    salvarTexto: vi.fn((arquivo: string, texto: string, versaoLida: string): Promise<SalvoTexto> => {
      const t = catalogo(arquivo);
      if (versaoDe(arquivo) !== versaoLida) {
        return Promise.reject(
          `${CONFLITO} ${arquivo} mudou desde que foi aberto (editado fora do app?); recarregue para ver a versão atual`,
        );
      }
      arquivos.set(arquivo, { texto, versao: `v${++contador}` });
      return Promise.resolve({
        estado: estadoDe(t),
        versao: versaoDe(arquivo),
        avisos: avisosFalsos(t, texto),
      });
    }),
    restaurarTexto: vi.fn((arquivo: string): Promise<LeituraTexto> => {
      const t = catalogo(arquivo);
      arquivos.set(arquivo, {
        texto: t.grupo === "regrasDoProjeto" ? padraoFalso(t) : null,
        versao: `v${++contador}`,
      });
      return Promise.resolve(leitura(arquivo));
    }),
    situacaoProtecao: vi.fn((_s: string) => Promise.resolve(protecao(false))),
    mudarProtecao: vi.fn((_s: string, ligada: boolean) => Promise.resolve(protecao(ligada))),
  } satisfies ApiConfiguracoes;
  /** Simula outro programa gravando o arquivo (muda a versão: o próximo salvar conflita). */
  const editarFora = (arquivo: string, texto: string) => {
    arquivos.set(arquivo, { texto, versao: `v${++contador}` });
  };
  return Object.assign(api, {
    editarFora,
    textoNaBase: (arquivo: string) => arquivos.get(arquivo)!.texto,
    /** O conteúdo do chaveiro falso (conta → token). */
    chaveiro,
    /** Senhas dos usuários de teste (`<projeto>/<id>` → senha). */
    senhasTeste,
    comandos,
    testes,
    /** Define o resultado dos próximos "Testar". */
    aoTestar: (f: () => TesteToken) => {
      proximoTeste = f;
    },
  });
}
