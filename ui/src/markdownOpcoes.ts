/**
 * Opções do `markdown-to-jsx` comuns a tudo que a tela renderiza ({@link DocumentoBase}
 * e a prévia do editor de textos): o HTML do arquivo vira texto (nada de HTML cru) e
 * o sanitizador embutido descarta URLs `javascript:` e `data:`.
 */
export const OPCOES_MARKDOWN_BASE = {
  disableParsingRawHTML: true,
  forceBlock: true,
} as const;
