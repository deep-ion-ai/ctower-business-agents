import type { ReactNode } from "react";
import "./componentes.css";

/** Os seis tons do selo (prancheta Padrões). */
export type TomSelo = "neutro" | "andamento" | "concluido" | "atencao" | "erro" | "agente";

/**
 * O tom vem do significado, nunca da tela (#406): "proposta do agente" é sempre o tom
 * agente, "aberta" sempre andamento, "aguardando revisão" sempre atenção. As chaves
 * estão em minúsculas; {@link tomDoSignificado} normaliza o texto antes de procurar.
 */
export const TOM_DO_SIGNIFICADO: Readonly<Record<string, TomSelo>> = {
  "proposta do agente": "agente",
  "sugestão do agente": "agente",
  "do agente": "agente",
  aberta: "andamento",
  aberto: "andamento",
  "em andamento": "andamento",
  "em análise": "andamento",
  "aguardando revisão": "atencao",
  "aguardando resposta": "atencao",
  aguardando: "atencao",
  "a revisar": "atencao",
  "a revalidar": "atencao",
  "em revisão": "atencao",
  pendente: "atencao",
  confirmada: "concluido",
  confirmado: "concluido",
  concluída: "concluido",
  concluído: "concluido",
  aprovada: "concluido",
  aprovado: "concluido",
  respondida: "concluido",
  "não confere": "erro",
  recusada: "erro",
  recusado: "erro",
  falhou: "erro",
  erro: "erro",
};

/** O tom de um significado ("aberta", "Proposta do agente"); sem entrada, neutro. */
export function tomDoSignificado(significado: string): TomSelo {
  return TOM_DO_SIGNIFICADO[significado.trim().toLowerCase()] ?? "neutro";
}

/**
 * Selo: uma forma só (pílula, sem borda), com o tom do significado. Sem `tom`, o tom
 * sai do próprio texto por {@link tomDoSignificado} ("aberta" → andamento).
 */
export function Selo({
  tom,
  titulo,
  children,
}: {
  tom?: TomSelo;
  /** Dica ao passar o mouse. */
  titulo?: string;
  children: ReactNode;
}) {
  const tomFinal = tom ?? (typeof children === "string" ? tomDoSignificado(children) : "neutro");
  return (
    <span className="selo" data-tom={tomFinal} title={titulo}>
      {children}
    </span>
  );
}
