import { useState, type ChangeEvent } from "react";
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

/** Rótulo do formato nos avisos. */
const NOME_DO_FORMATO: Record<FormatoExportacao, string> = {
  word: "Word",
  html: "HTML",
  markdown: "Markdown",
  csv: "CSV",
};

/**
 * Figuras dos diagramas do documento para o Word e o HTML (#118, #166). Um diagrama
 * que não desenha (Mermaid inválido, falha no canvas) fica sem figura: o documento
 * mostra aviso e código. Devolve as figuras e quantos diagramas ficaram sem.
 */
async function figurasDosDiagramas(
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
      // Sem figura, o documento leva o aviso e o código no lugar do diagrama.
    }
  }
  return { figuras, semFigura: blocos.length - figuras.length };
}

/**
 * Exportação do documento da análise para Word, HTML ou Markdown (#37, #10, #166),
 * com o diálogo de salvar do projeto. O documento sai autocontido: sem o frontmatter
 * interno, com as evidências como notas e, se marcado, os trechos citados. No Word e
 * no HTML, os diagramas Mermaid são desenhados aqui, em PNG, e vão como figuras
 * (#118). O HTML usa o template e os tokens CSS da base; os avisos deles aparecem
 * aqui, sem impedir a exportação.
 */
export function ExportacaoAnalise({
  api,
  projeto,
  analise,
  aprovada,
  csv = false,
  desenharPng = pngDoMermaid,
}: Props) {
  const [formato, setFormato] = useState<FormatoExportacao>("word");
  const [opcoes, setOpcoes] = useState<OpcoesExportacao>({
    evidencias: true,
    perguntas: true,
    trechos: true,
    codigo_diagramas: true,
  });
  const [aviso, setAviso] = useState<string | null>(null);
  const [avisosTemplate, setAvisosTemplate] = useState<string[]>([]);
  const [ocupado, setOcupado] = useState(false);
  const [feito, setFeito] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  const marcar = (campo: keyof OpcoesExportacao) => (e: ChangeEvent<HTMLInputElement>) =>
    setOpcoes((o) => ({ ...o, [campo]: e.target.checked }));

  const exportar = async () => {
    setErro(null);
    setFeito(null);
    setAviso(null);
    setAvisosTemplate([]);
    setOcupado(true);
    try {
      const destino = await api.escolherDestino(analise, formato);
      if (!destino) return;
      if (formato === "word" || formato === "html") {
        const { figuras, semFigura } = await figurasDosDiagramas(
          api,
          projeto,
          analise,
          desenharPng,
        );
        const feita = await api.exportar(projeto, analise, { destino, formato, opcoes, figuras });
        setFeito(feita.caminho);
        setAvisosTemplate(feita.avisos);
        const nome = NOME_DO_FORMATO[formato];
        if (semFigura > 0) {
          setAviso(
            semFigura === 1
              ? `Um diagrama não pôde ser desenhado: no ${nome} ele aparece como aviso e código.`
              : `${semFigura} diagramas não puderam ser desenhados: no ${nome} eles aparecem como aviso e código.`,
          );
        }
        return;
      }
      const feita = await api.exportar(projeto, analise, { destino, formato, opcoes });
      setFeito(feita.caminho);
      setAvisosTemplate(feita.avisos);
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
            checked={formato === "html"}
            onChange={() => setFormato("html")}
          />{" "}
          HTML (.html, abre no navegador)
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
            disabled={formato !== "word" && formato !== "html"}
            onChange={marcar("codigo_diagramas")}
          />{" "}
          Código dos diagramas (anexo do Word e do HTML)
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
      {avisosTemplate.length > 0 && (
        <div className="attention-text" role="status">
          <p>
            O template da exportação em HTML tem avisos (o arquivo foi gravado; corrija em
            Configurações, Modelos e prompts):
          </p>
          <ul>
            {avisosTemplate.map((a) => (
              <li key={a}>{a}</li>
            ))}
          </ul>
        </div>
      )}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
    </section>
  );
}
