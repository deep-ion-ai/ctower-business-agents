import { invoke } from "@tauri-apps/api/core";
import type { CriterioCitado, Fundamento, RastroTela } from "./analise";

/**
 * Visualizador do protótipo de telas (#382): o HTML gravado de cada tela
 * (`ui/src-tauri/src/prototipo.rs`) e a escolha do estado mostrado.
 */
export interface ApiVisualizador {
  /** HTML da tela `TL-<tela>`; `null` enquanto a tela não tem protótipo gravado. */
  htmlDaTela(projeto: string, analise: string, tela: number): Promise<string | null>;
}

export const apiVisualizadorTauri: ApiVisualizador = {
  htmlDaTela: (projeto, analise, tela) =>
    invoke<string | null>("html_da_tela_prototipo_app", { projeto, analise, tela }),
};

/** Estado principal de uma tela (`cba_core::prototipo_html::EstadoTela`). */
export type EstadoTela = "vazio" | "preenchido" | "erro" | "sucesso";

export const ESTADOS_TELA: { valor: EstadoTela; rotulo: string }[] = [
  { valor: "vazio", rotulo: "Vazio" },
  { valor: "preenchido", rotulo: "Preenchido" },
  { valor: "erro", rotulo: "Erro de validação" },
  { valor: "sucesso", rotulo: "Sucesso" },
];

/**
 * O documento da tela mostrando só o `estado` escolhido no app. O `iframe` sem
 * script não deixa o app mudar o fragmento (`#estado-erro`), então o estado vai
 * num `<style>` acrescentado ao documento, que esconde as outras seções e os
 * links de estado do cabeçalho. Sem estado, o documento vai como está: os quatro
 * estados empilhados e os links `#estado-*` de dentro funcionando.
 */
export function comEstado(html: string, estado: EstadoTela | null): string {
  if (!estado) return html;
  const estilo =
    `<style id="visualizador-estado">\n` +
    `.prototipo-tela section[data-estado]:not([data-estado="${estado}"]) { display: none !important; }\n` +
    `.prototipo-tela section[data-estado="${estado}"] { display: block !important; }\n` +
    `.prototipo-estados { display: none !important; }\n` +
    `</style>\n`;
  const fim = html.search(/<\/head>/i);
  return fim >= 0 ? html.slice(0, fim) + estilo + html.slice(fim) : estilo + html;
}

/** Referência e texto de um fundamento ou critério, para os chips de origem. */
export function referencias(r: RastroTela): { rotulo: string; texto: string }[] {
  const fundamento = (f: Fundamento) =>
    f.tipo === "afirmacao"
      ? { rotulo: `af. ${f.ref}`, texto: f.texto }
      : { rotulo: f.id, texto: f.titulo };
  const criterio = (c: CriterioCitado) => ({
    rotulo: `${c.historia}/CA-${c.criterio}`,
    texto: c.texto,
  });
  return [...(r.fundamentos ?? []).map(fundamento), ...(r.criterios ?? []).map(criterio)];
}

/** Sem fundamento nem critério, ou dependente de pergunta em aberto. */
export function aConfirmar(r: RastroTela): boolean {
  return !!r.pergunta || ((r.fundamentos ?? []).length === 0 && (r.criterios ?? []).length === 0);
}
