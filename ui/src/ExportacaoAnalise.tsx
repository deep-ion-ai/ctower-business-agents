import { useState, type ChangeEvent } from "react";
import { mensagemDe } from "./analise";
import type { ApiRevisao, FormatoExportacao, OpcoesExportacao } from "./revisao";

interface Props {
  api: ApiRevisao;
  sistema: string;
  analise: string;
  /** A análise já foi aprovada (fora disso, o documento sai como rascunho). */
  aprovada: boolean;
}

/**
 * Exportação do documento da análise para Markdown ou Word (#37, #10), com o
 * diálogo de salvar do sistema. O documento sai autocontido: sem o frontmatter
 * interno, com as evidências como notas e, se marcado, os trechos citados.
 */
export function ExportacaoAnalise({ api, sistema, analise, aprovada }: Props) {
  const [formato, setFormato] = useState<FormatoExportacao>("word");
  const [opcoes, setOpcoes] = useState<OpcoesExportacao>({
    evidencias: true,
    perguntas: true,
    trechos: true,
  });
  const [ocupado, setOcupado] = useState(false);
  const [feito, setFeito] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  const marcar = (campo: keyof OpcoesExportacao) => (e: ChangeEvent<HTMLInputElement>) =>
    setOpcoes((o) => ({ ...o, [campo]: e.target.checked }));

  const exportar = async () => {
    setErro(null);
    setFeito(null);
    setOcupado(true);
    try {
      const destino = await api.escolherDestino(analise, formato);
      if (!destino) return;
      setFeito(await api.exportar(sistema, analise, { destino, formato, opcoes }));
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
      </fieldset>
      <fieldset>
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
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
    </section>
  );
}
