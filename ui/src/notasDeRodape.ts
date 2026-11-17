/** Espaço de largura zero: separa duas notas coladas sem aparecer na tela. */
const SEPARADOR = "​";

/** Trechos em que o texto vale como está: blocos de código e código em linha. */
const CODIGO = /(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]+`)/;

/** Uma nota de rodapé (`[^1]`) seguida de outra, com ou sem espaço no meio. */
const NOTAS_SEGUIDAS = /(\[\^[^\]\s]+\])(?=[ \t]*\[\^[^\]\s]+\])/g;

/**
 * Prepara o Markdown para o renderizador mostrar todas as notas de rodapé de uma
 * sequência. O `markdown-to-jsx` lê `[^1][^2]` (ou `[^1] [^2]`) como link de
 * referência `[texto][rótulo]` e deixa o trecho cru na tela; um separador invisível
 * entre as notas desfaz essa leitura. O arquivo não muda: só o que vai ao renderizador.
 * Código (em bloco ou em linha) fica como está.
 */
export function separarNotasSeguidas(markdown: string): string {
  if (!markdown.includes("[^")) return markdown;
  return markdown
    .split(CODIGO)
    .map((trecho, i) => (i % 2 === 1 ? trecho : trecho.replace(NOTAS_SEGUIDAS, `$1${SEPARADOR}`)))
    .join("");
}
