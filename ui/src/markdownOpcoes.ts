import { RuleType, type MarkdownToJSX } from "markdown-to-jsx";
import { createElement } from "react";
import { DiagramaMermaid } from "./DiagramaMermaid";

/**
 * Bloco ` ```mermaid ` vira o diagrama desenhado (#118); os demais blocos seguem o
 * caminho padrão.
 */
const comDiagramas: MarkdownToJSX.Options["renderRule"] = (proximo, no, _filhos, estado) => {
  if (no.type === RuleType.codeBlock && no.lang?.trim().toLowerCase() === "mermaid") {
    return createElement(DiagramaMermaid, { key: estado.key, codigo: no.text });
  }
  return proximo();
};

/**
 * Opções do `markdown-to-jsx` comuns a tudo que a tela renderiza ({@link DocumentoBase}
 * e a prévia do editor de textos): o HTML do arquivo vira texto (nada de HTML cru), o
 * sanitizador embutido descarta URLs `javascript:` e `data:` e os blocos Mermaid
 * aparecem como diagrama.
 */
export const OPCOES_MARKDOWN_BASE = {
  disableParsingRawHTML: true,
  forceBlock: true,
  renderRule: comDiagramas,
} as const;
