import type { AmbienteClaude } from "./analise";
import type { AnaliseInicio, Inicio } from "./conhecimento";

/**
 * O próximo passo que o Início sugere (#227), do mais urgente ao menos:
 * o agente espera o analista, uma análise concluída pede revisão e aprovação,
 * regras a revalidar, ambiente do Claude por preparar, análise parada para retomar e,
 * num projeto sem análise, a primeira análise.
 */
export type PassoInicio =
  | { tipo: "responder"; analise: AnaliseInicio }
  | { tipo: "revisar"; analise: AnaliseInicio }
  | { tipo: "revalidar"; projeto: string; nome: string; regras: number }
  | { tipo: "preparar"; projeto: string; nome: string }
  | { tipo: "retomar"; analise: AnaliseInicio }
  | { tipo: "primeira-analise"; projeto: string; nome: string };

/** `undefined`: ainda não conferido; `null`: a conferência falhou. */
export type AmbientesConferidos = Record<string, AmbienteClaude | null | undefined>;

/** Análise que o analista ainda precisa tocar: não aprovada. */
export const naoAprovada = (a: AnaliseInicio) => a.estado !== "aprovada";

/** Concluída pelo agente e aguardando a revisão do analista. */
export const aguardaRevisao = (a: AnaliseInicio) => a.estado === "concluida";

/** Criada ou com o agente trabalhando: é o que “Em andamento” lista. */
export const emAndamento = (a: AnaliseInicio) => a.estado === "criada" || a.estado === "em-andamento";

/** Todas as análises conhecidas do início, sem repetir (as recentes repetem as em andamento). */
function todas(inicio: Inicio): AnaliseInicio[] {
  const vistas = new Set<string>();
  return [...inicio.emAndamento, ...inicio.recentes].filter((a) => {
    const k = `${a.projeto}/${a.id}`;
    if (vistas.has(k)) return false;
    vistas.add(k);
    return true;
  });
}

export function proximoPasso(inicio: Inicio, ambientes: AmbientesConferidos): PassoInicio | null {
  if (inicio.base === null || inicio.projetos.length === 0) return null;
  const analises = todas(inicio);

  const aguardando = analises.find(
    (a) => a.estado !== "aprovada" && a.sessao?.estado === "aguardando-analista",
  );
  if (aguardando) return { tipo: "responder", analise: aguardando };

  const revisar = analises.find(aguardaRevisao);
  if (revisar) return { tipo: "revisar", analise: revisar };

  const aRevalidar = inicio.projetos.find((p) => !p.erro && p.aRevalidar > 0);
  if (aRevalidar) {
    return {
      tipo: "revalidar",
      projeto: aRevalidar.id,
      nome: aRevalidar.nome,
      regras: aRevalidar.aRevalidar,
    };
  }

  const semPreparo = inicio.projetos.find((p) => !p.erro && ambientes[p.id]?.pronto === false);
  if (semPreparo) return { tipo: "preparar", projeto: semPreparo.id, nome: semPreparo.nome };

  const parada = analises.find(emAndamento);
  if (parada) return { tipo: "retomar", analise: parada };

  const vazio = inicio.projetos.find((p) => !p.erro && p.analises === 0);
  if (vazio) return { tipo: "primeira-analise", projeto: vazio.id, nome: vazio.nome };
  return null;
}
