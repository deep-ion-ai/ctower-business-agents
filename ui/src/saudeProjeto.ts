import type { AmbienteDetalhado, ProjetoConfig } from "./configuracoes";
import {
  repositorioDoDestino,
  type DestinoDocs,
  type IntegracaoProntidao,
  type Prontidao,
  type RepositorioCodigo,
  type TrackerConfig,
} from "./repositorios";

/**
 * Saúde de um projeto nas configurações (#192): o checklist da Visão geral, as marcas
 * das abas, o selo do cabeçalho e o nº de pendências do menu saem daqui, para os
 * quatro dizerem a mesma coisa.
 */
export type AbaProjeto =
  "geral" | "codigo" | "leitura" | "teste" | "agente" | "avancado";

export const ABAS_DO_PROJETO: { id: AbaProjeto; rotulo: string }[] = [
  { id: "geral", rotulo: "Visão geral" },
  { id: "codigo", rotulo: "Código e demandas" },
  { id: "leitura", rotulo: "O que é analisado" },
  { id: "teste", rotulo: "Ambiente de teste" },
  { id: "agente", rotulo: "Agente" },
  { id: "avancado", rotulo: "Avançado" },
];

/** `ok`: tudo certo; `problema`: conta como pendência; `neutro`: nada a fazer ou ainda conferindo. */
export type EstadoItem = "ok" | "problema" | "neutro";

export interface ItemSaude {
  id: "codigo" | "demandas" | "docs" | "leitura" | "agente";
  estado: EstadoItem;
  titulo: string;
  texto: string;
  /** Rótulo do botão que leva à aba certa. */
  acao: string;
  aba: AbaProjeto;
  /** O conserto é o token global do host: o botão leva a Configurações › Acessos. */
  acessos?: boolean;
}

/** `undefined`: ainda não conferido; `null`: a conferência falhou. */
export interface EstadoDoProjeto {
  ambiente?: AmbienteDetalhado | null;
  prontidao?: Prontidao | null;
}

export const FORJA: Record<RepositorioCodigo["forja"], string> = {
  github: "GitHub",
  gitlab: "GitLab",
};

export const ORIGEM_DEMANDAS: Record<TrackerConfig["kind"], string> = {
  github: "GitHub Issues",
  gitlab: "GitLab Issues",
  jira: "Jira",
};

/** Integração com algum passo que falhou. */
export function falhou(i: IntegracaoProntidao | undefined): boolean {
  return !!i?.passos.some((p) => p.situacao === "falhou");
}

/** O primeiro passo que falhou, com o detalhe (`Acesso de leitura: recusado`). */
export function primeiraFalha(
  i: IntegracaoProntidao | undefined,
): string | null {
  const p = i?.passos.find((x) => x.situacao === "falhou");
  if (!p) return null;
  return p.detalhe ? `${p.rotulo}: ${p.detalhe}` : p.rotulo;
}

/** Por que o teste de conexão das demandas falhou (ou `ok`). */
export type SituacaoDemandas =
  "ok" | "sem-acesso" | "issues-desligadas" | "nao-encontrado" | "falhou";

/**
 * Classifica o resultado do teste das demandas pelo passo que falhou: issues desligadas
 * e repositório não encontrado vêm do passo `issues` (o detalhe distingue os casos).
 */
export function situacaoDasDemandas(i: IntegracaoProntidao): SituacaoDemandas {
  const p = passoQueFalhou(i);
  if (!p) return "ok";
  const detalhe = (p.detalhe ?? "").toLowerCase();
  if (p.id === "issues") {
    if (detalhe.includes("desligadas")) return "issues-desligadas";
    if (detalhe.includes("não encontrado")) return "nao-encontrado";
    return "sem-acesso";
  }
  if (p.id === "credencial" || p.id === "token" || p.id === "acesso")
    return "sem-acesso";
  return "falhou";
}

export const ROTULO_SITUACAO_DEMANDAS: Record<SituacaoDemandas, string> = {
  ok: "conectado",
  "sem-acesso": "sem acesso",
  "issues-desligadas": "issues desligadas",
  "nao-encontrado": "repositório não encontrado",
  falhou: "com problema",
};

/** Frase do retorno do teste, em português. */
export function resumoDoTeste(i: IntegracaoProntidao, origem: string): string {
  const s = situacaoDasDemandas(i);
  if (s === "ok") return `Conexão com ${origem} funcionando.`;
  const motivo = passoQueFalhou(i)?.detalhe;
  const base: Record<Exclude<SituacaoDemandas, "ok">, string> = {
    "sem-acesso": `Sem acesso a ${origem}.`,
    "issues-desligadas": `As issues estão desligadas nesse repositório.`,
    "nao-encontrado": `Repositório não encontrado em ${origem}.`,
    falhou: `Não foi possível conectar a ${origem}.`,
  };
  return motivo
    ? `${base[s]} ${motivo.charAt(0).toUpperCase()}${motivo.slice(1)}.`
    : base[s];
}

/** `17/11/2026 14:05`. */
export function dataDoTeste(ms: number): string {
  return new Date(ms).toLocaleString("pt-BR", {
    dateStyle: "short",
    timeStyle: "short",
  });
}

/** O primeiro passo que falhou. */
function passoQueFalhou(i: IntegracaoProntidao | undefined) {
  return i?.passos.find((p) => p.situacao === "falhou");
}

/**
 * O passo que falhou é do token: a credencial, o token recusado pela API da forja
 * (`token`, #215) ou o acesso com ele.
 */
export function falhaDoToken(i: IntegracaoProntidao | undefined): boolean {
  const id = passoQueFalhou(i)?.id;
  return id === "credencial" || id === "token" || id === "acesso";
}

/** O problema é o próprio token (falta ou a API da forja o recusou), não a URL. */
function semTokenValido(i: IntegracaoProntidao | undefined): boolean {
  const id = passoQueFalhou(i)?.id;
  return id === "credencial" || id === "token";
}

/** As demandas usam o token global do host (o de Configurações › Acessos). */
function demandasComTokenDoHost(t: TrackerConfig): boolean {
  return (
    t.kind === "github" || (t.kind === "gitlab" && !t.credential_cmd.trim())
  );
}

/**
 * A prontidão traz uma integração por repositório de código, na ordem do cadastro, e
 * por último a das demandas (quando há).
 */
export function integracaoDoRepositorio(
  prontidao: Prontidao | null | undefined,
  indice: number,
): IntegracaoProntidao | undefined {
  return prontidao?.integracoes[indice];
}

export function integracaoDasDemandas(
  projeto: ProjetoConfig,
  prontidao: Prontidao | null | undefined,
): IntegracaoProntidao | undefined {
  if (!projeto.repositorios.tracker) return undefined;
  return prontidao?.integracoes[projeto.repositorios.codigo.length];
}

/**
 * O destino da documentação (#274) vem por último na prontidão, depois do código e das
 * demandas.
 */
export function integracaoDoDestino(
  projeto: ProjetoConfig,
  prontidao: Prontidao | null | undefined,
): IntegracaoProntidao | undefined {
  if (!projeto.repositorios.docs) return undefined;
  const i =
    projeto.repositorios.codigo.length + (projeto.repositorios.tracker ? 1 : 0);
  return prontidao?.integracoes[i];
}

/** `docs/negocio em GitHub dono/repo` ou `GitLab grupo/docs`. */
export function descreverDestino(
  docs: DestinoDocs | null | undefined,
  codigo: RepositorioCodigo[],
): string | null {
  if (!docs) return null;
  const r = repositorioDoDestino(docs, codigo);
  const onde = r
    ? `${FORJA[r.forja]} ${r.caminho}`
    : docs.modo === "subpasta"
      ? docs.codigo
      : "";
  const sub = (docs.subpasta ?? "").trim().replace(/^\/+|\/+$/g, "");
  return sub ? `${sub} em ${onde}` : onde;
}

/** Descrição curta da origem das demandas (`Jira FAT`, `GitHub Issues dono/repo`). */
export function descreverDemandas(t: TrackerConfig | null): string | null {
  if (!t) return null;
  if (t.kind === "jira") return `Jira ${t.project_key}`.trim();
  if (t.kind === "github") return `GitHub Issues ${t.repo}`.trim();
  return `GitLab Issues ${t.path}`.trim();
}

const plural = (n: number, um: string, varios: string) =>
  `${n} ${n === 1 ? um : varios}`;

/** “2 pastas · 1 repositório de código · demandas do Jira FAT”. */
export function resumoDoProjeto(projeto: ProjetoConfig): string {
  const partes = [
    plural(projeto.pastas.length, "pasta", "pastas"),
    plural(
      projeto.repositorios.codigo.length,
      "repositório de código",
      "repositórios de código",
    ),
  ];
  const demandas = descreverDemandas(projeto.repositorios.tracker);
  partes.push(demandas ? `demandas do ${demandas}` : "sem demandas conectadas");
  return partes.join(" · ");
}

export function itensDeSaude(
  projeto: ProjetoConfig,
  estado: EstadoDoProjeto,
): ItemSaude[] {
  const { ambiente, prontidao } = estado;
  const codigo = projeto.repositorios.codigo;

  let itemCodigo: ItemSaude;
  if (codigo.length === 0) {
    itemCodigo = {
      id: "codigo",
      estado: "neutro",
      titulo: "Nenhum repositório de código",
      texto:
        "Opcional: com o repositório, o app sabe de onde vem o código e qual branch usar.",
      acao: "Adicionar",
      aba: "codigo",
    };
  } else if (prontidao === undefined) {
    itemCodigo = {
      id: "codigo",
      estado: "neutro",
      titulo: "Conferindo o acesso ao código…",
      texto: plural(codigo.length, "repositório", "repositórios"),
      acao: "Ver repositórios",
      aba: "codigo",
    };
  } else {
    const comFalha = codigo
      .map((c, i) => {
        const integ = integracaoDoRepositorio(prontidao, i);
        return { c, integ, falha: primeiraFalha(integ) };
      })
      .filter((x) => x.falha !== null);
    // Token global do host ausente ou recusado pela API da forja: a pendência leva a
    // Acessos, mesmo que o repositório seja público (#215).
    const doHost = comFalha.find(
      (x) => x.c.token === "global" && semTokenValido(x.integ),
    );
    const recusadoNoHost =
      doHost && passoQueFalhou(doHost.integ)?.id === "token" ? doHost : null;
    itemCodigo =
      prontidao === null
        ? {
            id: "codigo",
            estado: "neutro",
            titulo: "Não foi possível conferir o acesso ao código",
            texto: "Confira de novo na aba Código e demandas.",
            acao: "Ver repositórios",
            aba: "codigo",
          }
        : recusadoNoHost
          ? {
              id: "codigo",
              estado: "problema",
              titulo: `O token de ${recusadoNoHost.c.host} foi recusado`,
              texto: `${recusadoNoHost.c.caminho}: ${recusadoNoHost.falha}.`,
              acao: "Abrir Acessos",
              aba: "codigo",
              acessos: true,
            }
          : comFalha.length > 0
            ? {
                id: "codigo",
                estado: "problema",
                titulo:
                  comFalha.length === 1
                    ? "Um repositório está sem acesso"
                    : `${comFalha.length} repositórios estão sem acesso`,
                texto: `${comFalha[0].c.caminho}: ${comFalha[0].falha}.`,
                acao: doHost ? "Abrir Acessos" : "Resolver",
                aba: "codigo",
                acessos: !!doHost,
              }
            : {
                id: "codigo",
                estado: "ok",
                titulo: "Código acessível",
                texto: `${plural(codigo.length, "repositório conferido", "repositórios conferidos")} ao abrir.`,
                acao: "Ver repositórios",
                aba: "codigo",
              };
  }

  const tracker = projeto.repositorios.tracker;
  const integ = integracaoDasDemandas(projeto, prontidao);
  const itemDemandas: ItemSaude = !tracker
    ? {
        id: "demandas",
        estado: "neutro",
        titulo: "Demandas não conectadas",
        texto: "Opcional: conecte o GitHub Issues, o GitLab Issues ou o Jira.",
        acao: "Conectar",
        aba: "codigo",
      }
    : falhou(integ)
      ? {
          id: "demandas",
          estado: "problema",
          titulo: "As demandas não estão acessíveis",
          texto: `${descreverDemandas(tracker)}: ${primeiraFalha(integ)}.`,
          ...(demandasComTokenDoHost(tracker) && semTokenValido(integ)
            ? { acao: "Abrir Acessos", acessos: true }
            : { acao: "Resolver" }),
          aba: "codigo",
        }
      : {
          id: "demandas",
          estado: integ ? "ok" : "neutro",
          titulo: integ ? "Demandas conectadas" : "Conferindo as demandas…",
          texto: descreverDemandas(tracker) ?? "",
          acao: "Ver",
          aba: "codigo",
        };

  // O destino da documentação só entra no checklist quando está cadastrado (#274).
  const docs = projeto.repositorios.docs;
  const integDocs = integracaoDoDestino(projeto, prontidao);
  const textoDocs = descreverDestino(docs, codigo) ?? "";
  const itemDocs: ItemSaude | null = !docs
    ? null
    : falhou(integDocs)
      ? {
          id: "docs",
          estado: "problema",
          titulo: "O destino da documentação não está acessível",
          texto: `${textoDocs}: ${primeiraFalha(integDocs)}.`,
          ...(docs.modo === "repositorio" &&
          docs.repositorio.token === "global" &&
          semTokenValido(integDocs)
            ? { acao: "Abrir Acessos", acessos: true }
            : { acao: "Resolver" }),
          aba: "codigo",
        }
      : {
          id: "docs",
          estado: integDocs ? "ok" : "neutro",
          titulo: integDocs
            ? "Destino da documentação acessível"
            : prontidao === null
              ? "Não foi possível conferir o destino da documentação"
              : "Conferindo o destino da documentação…",
          texto: textoDocs,
          acao: "Ver",
          aba: "codigo",
        };

  const sumidas = projeto.pastas.filter((p) => !p.existe);
  const itemLeitura: ItemSaude =
    sumidas.length > 0
      ? {
          id: "leitura",
          estado: "problema",
          titulo:
            sumidas.length === 1
              ? "Uma pasta não existe mais"
              : `${sumidas.length} pastas não existem mais`,
          texto: `${sumidas[0].caminho}: remova-a ou aponte a pasta nova.`,
          acao: "Ajustar",
          aba: "leitura",
        }
      : {
          id: "leitura",
          estado: "ok",
          titulo: "O que o agente lê",
          texto:
            `${plural(projeto.pastas.length, "pasta", "pastas")}` +
            (projeto.exclusoes.length > 0
              ? `; ${plural(projeto.exclusoes.length, "exclusão sua", "exclusões suas")}.`
              : "; só as exclusões padrão (segredos, dependências, binários)."),
          acao: "Ajustar",
          aba: "leitura",
        };

  const itemAgente: ItemSaude =
    ambiente === undefined
      ? {
          id: "agente",
          estado: "neutro",
          titulo: "Verificando o agente…",
          texto: "O app confere a configuração do Claude deste projeto.",
          acao: "Ver",
          aba: "agente",
        }
      : ambiente === null
        ? {
            id: "agente",
            estado: "neutro",
            titulo: "Não foi possível verificar o agente",
            texto: "Veja os detalhes na aba Agente.",
            acao: "Ver",
            aba: "agente",
          }
        : ambiente.pronto
          ? {
              id: "agente",
              estado: "ok",
              titulo: "Agente preparado",
              texto:
                "Ferramentas do app conectadas ao Claude e avisos automáticos ligados.",
              acao: "Ver",
              aba: "agente",
            }
          : {
              id: "agente",
              estado: "problema",
              titulo: "Agente ainda não preparado",
              texto:
                "Sem o preparo, o agente não consegue registrar afirmações no documento.",
              acao: "Preparar",
              aba: "agente",
            };

  return [
    itemCodigo,
    itemDemandas,
    ...(itemDocs ? [itemDocs] : []),
    itemLeitura,
    itemAgente,
  ];
}

/**
 * Pendências do projeto para o menu: `null` enquanto o ambiente não foi verificado
 * (ou a verificação falhou), para o menu não inventar “tudo pronto”.
 */
export function pendenciasDoProjeto(
  projeto: ProjetoConfig,
  estado: EstadoDoProjeto,
): number | null {
  if (!estado.ambiente) return null;
  return itensDeSaude(projeto, estado).filter((i) => i.estado === "problema")
    .length;
}

/** Marca de cada aba: o nº de pendências na Visão geral, “!” nas abas com problema. */
export function marcasDasAbas(
  itens: ItemSaude[],
): Partial<Record<AbaProjeto, string>> {
  const problemas = itens.filter((i) => i.estado === "problema");
  const marcas: Partial<Record<AbaProjeto, string>> = {};
  if (problemas.length > 0) marcas.geral = String(problemas.length);
  for (const p of problemas) marcas[p.aba] = "!";
  return marcas;
}
