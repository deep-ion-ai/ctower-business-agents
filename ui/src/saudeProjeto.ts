import type { AmbienteDetalhado, ProjetoConfig } from "./configuracoes";
import type {
  IntegracaoProntidao,
  Prontidao,
  RepositorioCodigo,
  TrackerConfig,
} from "./repositorios";

/**
 * Saúde de um projeto nas configurações (#192): o checklist da Visão geral, as marcas
 * das abas, o selo do cabeçalho e o nº de pendências do menu saem daqui, para os
 * quatro dizerem a mesma coisa.
 */
export type AbaProjeto = "geral" | "codigo" | "leitura" | "agente" | "avancado";

export const ABAS_DO_PROJETO: { id: AbaProjeto; rotulo: string }[] = [
  { id: "geral", rotulo: "Visão geral" },
  { id: "codigo", rotulo: "Código e demandas" },
  { id: "leitura", rotulo: "O que é analisado" },
  { id: "agente", rotulo: "Agente" },
  { id: "avancado", rotulo: "Avançado" },
];

/** `ok`: tudo certo; `problema`: conta como pendência; `neutro`: nada a fazer ou ainda conferindo. */
export type EstadoItem = "ok" | "problema" | "neutro";

export interface ItemSaude {
  id: "codigo" | "demandas" | "leitura" | "agente";
  estado: EstadoItem;
  titulo: string;
  texto: string;
  /** Rótulo do botão que leva à aba certa. */
  acao: string;
  aba: AbaProjeto;
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
export function primeiraFalha(i: IntegracaoProntidao | undefined): string | null {
  const p = i?.passos.find((x) => x.situacao === "falhou");
  if (!p) return null;
  return p.detalhe ? `${p.rotulo}: ${p.detalhe}` : p.rotulo;
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

/** Descrição curta da origem das demandas (`Jira FAT`, `GitHub Issues dono/repo`). */
export function descreverDemandas(t: TrackerConfig | null): string | null {
  if (!t) return null;
  if (t.kind === "jira") return `Jira ${t.project_key}`.trim();
  if (t.kind === "github") return `GitHub Issues ${t.repo}`.trim();
  return `GitLab Issues ${t.path}`.trim();
}

const plural = (n: number, um: string, varios: string) => `${n} ${n === 1 ? um : varios}`;

/** “2 pastas · 1 repositório de código · demandas do Jira FAT”. */
export function resumoDoProjeto(projeto: ProjetoConfig): string {
  const partes = [
    plural(projeto.pastas.length, "pasta", "pastas"),
    plural(projeto.repositorios.codigo.length, "repositório de código", "repositórios de código"),
  ];
  const demandas = descreverDemandas(projeto.repositorios.tracker);
  partes.push(demandas ? `demandas do ${demandas}` : "sem demandas conectadas");
  return partes.join(" · ");
}

export function itensDeSaude(projeto: ProjetoConfig, estado: EstadoDoProjeto): ItemSaude[] {
  const { ambiente, prontidao } = estado;
  const codigo = projeto.repositorios.codigo;

  let itemCodigo: ItemSaude;
  if (codigo.length === 0) {
    itemCodigo = {
      id: "codigo",
      estado: "neutro",
      titulo: "Nenhum repositório de código",
      texto: "Opcional: com o repositório, o app sabe de onde vem o código e qual branch usar.",
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
      .map((c, i) => ({ c, falha: primeiraFalha(integracaoDoRepositorio(prontidao, i)) }))
      .filter((x) => x.falha !== null);
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
        : comFalha.length > 0
          ? {
              id: "codigo",
              estado: "problema",
              titulo:
                comFalha.length === 1
                  ? "Um repositório está sem acesso"
                  : `${comFalha.length} repositórios estão sem acesso`,
              texto: `${comFalha[0].c.caminho}: ${comFalha[0].falha}.`,
              acao: "Resolver",
              aba: "codigo",
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
          acao: "Resolver",
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
              texto: "Ferramentas do app conectadas ao Claude e avisos automáticos ligados.",
              acao: "Ver",
              aba: "agente",
            }
          : {
              id: "agente",
              estado: "problema",
              titulo: "Agente ainda não preparado",
              texto: "Sem o preparo, o agente não consegue registrar afirmações no documento.",
              acao: "Preparar",
              aba: "agente",
            };

  return [itemCodigo, itemDemandas, itemLeitura, itemAgente];
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
  return itensDeSaude(projeto, estado).filter((i) => i.estado === "problema").length;
}

/** Marca de cada aba: o nº de pendências na Visão geral, “!” nas abas com problema. */
export function marcasDasAbas(itens: ItemSaude[]): Partial<Record<AbaProjeto, string>> {
  const problemas = itens.filter((i) => i.estado === "problema");
  const marcas: Partial<Record<AbaProjeto, string>> = {};
  if (problemas.length > 0) marcas.geral = String(problemas.length);
  for (const p of problemas) marcas[p.aba] = "!";
  return marcas;
}
