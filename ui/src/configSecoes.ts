import { useCallback, useState } from "react";

/**
 * Seções do menu lateral das configurações (#191). "projeto" é uma só seção, com o
 * projeto escolhido à parte ({@link Escolha.projeto}).
 */
export type SecaoConfig = "aparencia" | "assistente" | "acessos" | "pasta" | "modelos" | "projeto";

export const GRUPOS_DO_MENU: { titulo: string; secoes: { id: SecaoConfig; rotulo: string }[] }[] = [
  {
    titulo: "App",
    secoes: [
      { id: "aparencia", rotulo: "Aparência" },
      { id: "assistente", rotulo: "Assistente de IA" },
      { id: "acessos", rotulo: "Acessos" },
    ],
  },
  {
    titulo: "Base de conhecimento",
    secoes: [
      { id: "pasta", rotulo: "Pasta da base" },
      { id: "modelos", rotulo: "Modelos e prompts" },
    ],
  },
];

/**
 * Seções que o menu esconde enquanto não existem. Hoje nenhuma: "Acessos" (#193) já
 * existe. Uma seção nova ainda sem tela entra aqui até ficar pronta.
 */
export const SECOES_OCULTAS: readonly SecaoConfig[] = [];

export interface Escolha {
  secao: SecaoConfig;
  /** Projeto aberto, quando `secao` é "projeto". */
  projeto: string | null;
}

export const ESCOLHA_INICIAL: Escolha = { secao: "aparencia", projeto: null };

const CHAVE = "cba.configuracoes.secao";

const SECOES_SIMPLES: readonly SecaoConfig[] = [
  "aparencia",
  "assistente",
  "acessos",
  "pasta",
  "modelos",
];

/** `aparencia`, `pasta`… ou `projeto:<id>`; qualquer outra coisa vale como ausente. */
export function lerEscolha(texto: string | null): Escolha | null {
  if (!texto) return null;
  if (texto.startsWith("projeto:") && texto.length > "projeto:".length) {
    return { secao: "projeto", projeto: texto.slice("projeto:".length) };
  }
  const secao = SECOES_SIMPLES.find((s) => s === texto);
  return secao ? { secao, projeto: null } : null;
}

export function textoDaEscolha(e: Escolha): string {
  return e.secao === "projeto" && e.projeto ? `projeto:${e.projeto}` : e.secao;
}

function guardada(): Escolha {
  try {
    return lerEscolha(localStorage.getItem(CHAVE)) ?? ESCOLHA_INICIAL;
  } catch {
    return ESCOLHA_INICIAL;
  }
}

/** A seção escolhida, lembrada ao sair e voltar para as configurações (e entre aberturas do app). */
export function useEscolhaLembrada() {
  const [escolha, setEscolha] = useState<Escolha>(guardada);
  const escolher = useCallback((nova: Escolha) => {
    setEscolha(nova);
    try {
      localStorage.setItem(CHAVE, textoDaEscolha(nova));
    } catch {
      // Sem armazenamento: a escolha vale só enquanto a tela está aberta.
    }
  }, []);
  return [escolha, escolher] as const;
}

/**
 * A escolha que a tela mostra de fato: uma seção oculta ou um projeto que não existe
 * mais (descadastrado, outra base) cai no primeiro projeto, ou na Aparência sem projetos.
 */
export function escolhaEfetiva(e: Escolha, projetos: { id: string }[]): Escolha {
  if (e.secao === "projeto") {
    const achado = projetos.find((p) => p.id === e.projeto) ?? projetos[0];
    return achado ? { secao: "projeto", projeto: achado.id } : ESCOLHA_INICIAL;
  }
  return SECOES_OCULTAS.includes(e.secao) ? ESCOLHA_INICIAL : e;
}
