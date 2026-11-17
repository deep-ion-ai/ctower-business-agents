import { useEffect, useState, type ChangeEvent } from "react";
import { mensagemDe } from "./analise";
import { pngDoMermaid, type FiguraPng } from "./mermaid";
import type { ApiRevisao, FiguraPedida, FormatoExportacao, OpcoesExportacao } from "./revisao";

interface Props {
  api: ApiRevisao;
  projeto: string;
  analise: string;
  /** A análise já foi aprovada (fora disso, o documento sai como rascunho). */
  aprovada: boolean;
  /** Oferece o CSV com uma linha por cenário (análise de cenários de teste, #117). */
  csv?: boolean;
  /** Desenha um diagrama em PNG (testes trocam; o padrão usa o Mermaid empacotado). */
  desenharPng?: (codigo: string) => Promise<FiguraPng>;
}

const CHAVE_PREFERENCIA = "cba.exportacao";
const PADRAO: { formato: FormatoExportacao; opcoes: OpcoesExportacao } = {
  formato: "word",
  opcoes: { evidencias: true, perguntas: true, trechos: true, codigo_diagramas: true },
};

/** Última escolha de formato e opções do Exportar; qualquer falha cai no padrão. */
function lerPreferencia(): typeof PADRAO {
  try {
    const bruto = JSON.parse(localStorage.getItem(CHAVE_PREFERENCIA) ?? "null") as {
      formato?: unknown;
      opcoes?: Record<string, unknown>;
    } | null;
    if (!bruto || typeof bruto !== "object") return PADRAO;
    const opcoes = { ...PADRAO.opcoes };
    for (const k of Object.keys(opcoes) as (keyof OpcoesExportacao)[]) {
      if (typeof bruto.opcoes?.[k] === "boolean") opcoes[k] = bruto.opcoes[k] as boolean;
    }
    const formato = bruto.formato === "markdown" || bruto.formato === "word" || bruto.formato === "csv" ? bruto.formato : PADRAO.formato;
    return { formato: formato as FormatoExportacao, opcoes };
  } catch {
    return PADRAO;
  }
}

/**
 * Figuras dos diagramas do documento para o Word (#118). Um diagrama que não desenha
 * (Mermaid inválido, falha no canvas) fica sem figura: o Word mostra aviso e código.
 * Devolve as figuras e quantos diagramas ficaram sem.
 */
async function figurasDoWord(
  api: ApiRevisao,
  projeto: string,
  analise: string,
  desenhar: (codigo: string) => Promise<FiguraPng>,
): Promise<{ figuras: FiguraPedida[]; semFigura: number }> {
  const blocos = (await api.diagramasExportacao?.(projeto, analise)) ?? [];
  const figuras: FiguraPedida[] = [];
  for (const codigo of blocos) {
    try {
      const f = await desenhar(codigo);
      figuras.push({ codigo, png: Array.from(f.png), largura: f.largura, altura: f.altura });
    } catch {
      // Sem figura, o Word leva o aviso e o código no lugar do diagrama.
    }
  }
  return { figuras, semFigura: blocos.length - figuras.length };
}

/**
 * Exportação do documento da análise para Markdown ou Word (#37, #10), com o
 * diálogo de salvar do projeto. O documento sai autocontido: sem o frontmatter
 * interno, com as evidências como notas e, se marcado, os trechos citados. No Word, os
 * diagramas Mermaid são desenhados aqui, em PNG, e vão como figuras (#118).
 */
export function ExportacaoAnalise({
  api,
  projeto,
  analise,
  aprovada,
  csv = false,
  desenharPng = pngDoMermaid,
}: Props) {
  const [lembrado] = useState(lerPreferencia);
  const [formato, setFormato] = useState<FormatoExportacao>(
    lembrado.formato === "csv" && !csv ? "word" : lembrado.formato,
  );
  const [opcoes, setOpcoes] = useState<OpcoesExportacao>(lembrado.opcoes);
  useEffect(() => {
    try {
      localStorage.setItem(CHAVE_PREFERENCIA, JSON.stringify({ formato, opcoes }));
    } catch {
      // Sem armazenamento (janela privada, dados bloqueados): a escolha só vale nesta tela.
    }
  }, [formato, opcoes]);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [feito, setFeito] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  const marcar = (campo: keyof OpcoesExportacao) => (e: ChangeEvent<HTMLInputElement>) =>
    setOpcoes((o) => ({ ...o, [campo]: e.target.checked }));

  const exportar = async () => {
    setErro(null);
    setFeito(null);
    setAviso(null);
    setOcupado(true);
    try {
      const destino = await api.escolherDestino(analise, formato);
      if (!destino) return;
      if (formato === "word") {
        const { figuras, semFigura } = await figurasDoWord(api, projeto, analise, desenharPng);
        setFeito(await api.exportar(projeto, analise, { destino, formato, opcoes, figuras }));
        if (semFigura > 0) {
          setAviso(
            semFigura === 1
              ? "Um diagrama não pôde ser desenhado: no Word ele aparece como aviso e código."
              : `${semFigura} diagramas não puderam ser desenhados: no Word eles aparecem como aviso e código.`,
          );
        }
        return;
      }
      setFeito(await api.exportar(projeto, analise, { destino, formato, opcoes }));
    } catch (e) {
      setErro(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  return (
    <section className="exportacao" aria-label="Exportação">
      <h3>Exportar</h3>
      {!aprovada && (
        <p className="muted">A análise ainda não foi aprovada: o documento sai como rascunho.</p>
      )}
      <fieldset>
        <legend>Formato</legend>
        <label>
          <input
            type="radio"
            name="formato-exportacao"
            checked={formato === "word"}
            onChange={() => setFormato("word")}
          />{" "}
          Word (.docx)
        </label>{" "}
        <label>
          <input
            type="radio"
            name="formato-exportacao"
            checked={formato === "markdown"}
            onChange={() => setFormato("markdown")}
          />{" "}
          Markdown (.md)
        </label>
        {csv && (
          <>
            {" "}
            <label>
              <input
                type="radio"
                name="formato-exportacao"
                checked={formato === "csv"}
                onChange={() => setFormato("csv")}
              />{" "}
              CSV, um cenário por linha (.csv)
            </label>
          </>
        )}
      </fieldset>
      <fieldset disabled={formato === "csv"}>
        <legend>Incluir</legend>
        <label>
          <input type="checkbox" checked={opcoes.evidencias} onChange={marcar("evidencias")} />{" "}
          Evidências
        </label>{" "}
        <label>
          <input
            type="checkbox"
            checked={opcoes.trechos}
            disabled={!opcoes.evidencias}
            onChange={marcar("trechos")}
          />{" "}
          Trechos de código citados
        </label>{" "}
        <label>
          <input type="checkbox" checked={opcoes.perguntas} onChange={marcar("perguntas")} />{" "}
          Perguntas em aberto
        </label>{" "}
        <label>
          <input
            type="checkbox"
            checked={opcoes.codigo_diagramas}
            disabled={formato !== "word"}
            onChange={marcar("codigo_diagramas")}
          />{" "}
          Código dos diagramas (anexo do Word)
        </label>
      </fieldset>
      <button className="btn" type="button" disabled={ocupado} onClick={exportar}>
        Exportar…
      </button>
      {feito && (
        <p role="status">
          Exportado para <code>{feito}</code>
        </p>
      )}
      {aviso && <p className="attention-text">{aviso}</p>}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
    </section>
  );
}
