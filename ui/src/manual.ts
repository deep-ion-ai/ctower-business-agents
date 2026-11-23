import { invoke } from "@tauri-apps/api/core";

/**
 * Manual do usuário (#272): os prints dos passos (`ui/src-tauri/src/manual.rs`).
 * O texto dos passos, a dispensa e a legenda do print vão pelas decisões da
 * revisão (`DecisaoRevisao`).
 */
export interface ApiManual {
  /** Inclui ou troca o print do passo (PNG). */
  anexarPrint: (
    projeto: string,
    analise: string,
    passo: number,
    png: Uint8Array,
    legenda: string,
    textoAlternativo: string,
  ) => Promise<void>;
  removerPrint: (projeto: string, analise: string, passo: number) => Promise<void>;
  removerPasso: (projeto: string, analise: string, passo: number) => Promise<void>;
  /** Bytes do PNG, pelo caminho relativo à raiz do projeto na base. */
  lerPrint: (projeto: string, arquivo: string) => Promise<ArrayBuffer>;
}

export const apiManualTauri: ApiManual = {
  anexarPrint: (projeto, analise, passo, png, legenda, textoAlternativo) =>
    invoke("anexar_print_manual_app", {
      projeto,
      analise,
      passo,
      print: { png: Array.from(png), legenda, textoAlternativo },
    }),
  removerPrint: (projeto, analise, passo) =>
    invoke("remover_print_manual_app", { projeto, analise, passo }),
  removerPasso: (projeto, analise, passo) =>
    invoke("remover_passo_manual_app", { projeto, analise, passo }),
  lerPrint: (projeto, arquivo) =>
    invoke<ArrayBuffer>("ler_print_manual_app", { projeto, documento: null, destino: arquivo }),
};

/** Tamanho máximo de um print, como no app (`TAMANHO_MAXIMO_DO_PRINT`). */
export const TAMANHO_MAXIMO_DO_PRINT = 8 * 1024 * 1024;

const ASSINATURA_PNG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/** Os bytes começam pela assinatura do PNG. */
export function ePng(bytes: Uint8Array): boolean {
  return ASSINATURA_PNG.every((b, i) => bytes[i] === b);
}

/**
 * O arquivo colado, escolhido ou arrastado como PNG: o PNG vai como está; os
 * outros formatos de imagem (JPEG, WebP, GIF) são desenhados num canvas e
 * convertidos. Recusa o que não é imagem e o que passa do tamanho máximo.
 */
export async function comoPng(arquivo: Blob): Promise<Uint8Array> {
  const bytes = await lerBytes(arquivo);
  if (ePng(bytes)) return verificarTamanho(bytes);
  if (!arquivo.type.startsWith("image/")) {
    throw new Error("Escolha uma imagem (PNG, JPEG, WebP ou GIF).");
  }
  const bitmap = await createImageBitmap(arquivo);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Não foi possível converter a imagem para PNG.");
  ctx.drawImage(bitmap, 0, 0);
  const png = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, "image/png"));
  if (!png) throw new Error("Não foi possível converter a imagem para PNG.");
  return verificarTamanho(await lerBytes(png));
}

/** Bytes do arquivo (com `FileReader` onde `Blob.arrayBuffer` não existe). */
function lerBytes(arquivo: Blob): Promise<Uint8Array> {
  if (typeof arquivo.arrayBuffer === "function") {
    return arquivo.arrayBuffer().then((b) => new Uint8Array(b));
  }
  return new Promise((ok, falha) => {
    const leitor = new FileReader();
    leitor.onload = () => ok(new Uint8Array(leitor.result as ArrayBuffer));
    leitor.onerror = () => falha(leitor.error ?? new Error("Não foi possível ler o arquivo."));
    leitor.readAsArrayBuffer(arquivo);
  });
}

function verificarTamanho(bytes: Uint8Array): Uint8Array {
  if (bytes.length > TAMANHO_MAXIMO_DO_PRINT) {
    throw new Error(
      `O print passa de ${TAMANHO_MAXIMO_DO_PRINT / 1024 / 1024} MiB; recorte só a tela e tente de novo.`,
    );
  }
  return bytes;
}

/**
 * Caminho, relativo à raiz da base, da imagem `href` citada no documento
 * `documento` (também relativo à raiz da base). `null` se não aponta para um PNG
 * da base.
 */
export function resolverImagem(documento: string, href: string): string | null {
  let alvo: string;
  try {
    alvo = decodeURI(href.split("#")[0].split("?")[0]).replace(/\\/g, "/");
  } catch {
    return null;
  }
  if (!alvo || /^[a-z][a-z0-9+.-]*:/i.test(alvo) || alvo.startsWith("//") || alvo.startsWith("/")) {
    return null;
  }
  if (!alvo.toLowerCase().endsWith(".png")) return null;
  const partes = documento.split("/").slice(0, -1);
  for (const p of alvo.split("/")) {
    if (p === "" || p === ".") continue;
    if (p === "..") {
      if (partes.length === 0) return null;
      partes.pop();
    } else {
      partes.push(p);
    }
  }
  return partes.length > 1 ? partes.join("/") : null;
}

/** Aviso fixo ao anexar um print: a base pode ir para um repositório. */
export const AVISO_DADOS_SENSIVEIS =
  "Antes de anexar, confira se o print não mostra dados pessoais ou sigilosos (nomes, CPF, valores de clientes reais): a base de conhecimento pode ir para um repositório. Use dados de teste ou cubra o que for sensível.";
