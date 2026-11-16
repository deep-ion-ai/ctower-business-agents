import { useState, type ReactNode } from "react";
import type { Diagrama, Evidencia } from "./analise";
import { DiagramaMermaid } from "./DiagramaMermaid";
import type { ElementoDoDiagrama } from "./mermaid";

/** Linha da tabela de elementos: `no-<índice>` ou `aresta-<índice>`. */
type Linha = `no-${number}` | `aresta-${number}`;

const NOMES = {
  estados: { no: "Estado", aresta: "Transição", rotulo: "Evento" },
  fluxo: { no: "Passo", aresta: "Caminho", rotulo: "Condição" },
} as const;

const PAPEL: Record<string, string> = {
  inicio: "início",
  fim: "fim",
  decisao: "decisão",
};

const CONFIANCA: Record<string, string> = {
  confirmado: "confirmado",
  inferido: "inferido",
  a_confirmar: "a confirmar",
};

function chave(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

/**
 * Diagrama registrado pelo agente, no documento ao vivo (#118): a figura gerada pelo
 * app e a tabela de elementos, cada um com as evidências. Clicar num estado ou numa
 * transição da figura seleciona a linha da tabela e abre o trecho da primeira
 * evidência, como as notas de rodapé das afirmações.
 */
export function DiagramaAoVivo({
  diagrama,
  mermaid,
  evidenciaAberta,
  aoAbrirEvidencia,
  trecho,
  trechoAberto,
}: {
  diagrama: Diagrama;
  /** Código Mermaid gerado pelo app; sem ele (resposta antiga), só a tabela. */
  mermaid?: string;
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia) => void;
  /** Trecho da evidência aberta, mostrado abaixo da tabela quando `trechoAberto`. */
  trecho?: ReactNode;
  trechoAberto: boolean;
}) {
  const [selecionada, setSelecionada] = useState<Linha | null>(null);
  const nomes = NOMES[diagrama.tipo] ?? NOMES.fluxo;
  const arestas = diagrama.arestas ?? [];
  const rotuloDe = (c: string) => diagrama.nos.find((n) => n.chave === c)?.rotulo ?? c;

  const selecionar = (linha: Linha, evidencias: Evidencia[] | undefined) => {
    setSelecionada(linha);
    const primeira = evidencias?.[0];
    if (primeira && evidenciaAberta !== chave(primeira)) aoAbrirEvidencia(primeira);
  };

  const aoClicar = (el: ElementoDoDiagrama) => {
    if (el.tipo === "no") {
      const no = diagrama.nos[el.indice];
      if (no) selecionar(`no-${el.indice}`, no.evidencias);
      return;
    }
    const de = diagrama.nos[el.de]?.chave;
    const para = diagrama.nos[el.para]?.chave;
    const candidatas = arestas
      .map((a, i) => ({ a, i }))
      .filter(({ a }) => a.de === de && a.para === para);
    const escolhida = candidatas[el.ocorrencia] ?? candidatas[0];
    if (escolhida) selecionar(`aresta-${escolhida.i}`, escolhida.a.evidencias);
  };

  const botoes = (evidencias: Evidencia[] | undefined) =>
    (evidencias ?? []).map((e) => {
      const k = chave(e);
      return (
        <button
          key={k}
          type="button"
          className="btn btn-sm evidencia"
          aria-pressed={evidenciaAberta === k}
          title={e.trecho}
          onClick={() => aoAbrirEvidencia(e)}
        >
          {k}
        </button>
      );
    });

  return (
    <div className="diagrama-ao-vivo" data-testid={`diagrama-${diagrama.numero}`}>
      <p className="legenda-diagrama">
        <strong>D-{diagrama.numero}</strong> · {diagrama.titulo}{" "}
        <span className="muted">
          · gerado pelo app a partir dos elementos registrados
          {mermaid && " · clique num elemento para ver o trecho"}
        </span>
      </p>
      {mermaid && (
        <DiagramaMermaid codigo={mermaid} titulo={diagrama.titulo} aoClicar={aoClicar} />
      )}
      <table className="tabela-diagrama" aria-label={`Elementos de ${diagrama.titulo}`}>
        <thead>
          <tr>
            <th scope="col">Elemento</th>
            <th scope="col">{nomes.rotulo}</th>
            <th scope="col">Descrição</th>
            <th scope="col">Confiança</th>
            <th scope="col">Evidências</th>
          </tr>
        </thead>
        <tbody>
          {diagrama.nos.map((n, i) => (
            <tr
              key={`no-${n.chave}`}
              aria-selected={selecionada === `no-${i}`}
              data-selecionada={selecionada === `no-${i}` || undefined}
            >
              <td>
                {nomes.no} <strong>{n.rotulo}</strong>
                {n.papel && PAPEL[n.papel] && <span className="muted"> ({PAPEL[n.papel]})</span>}
              </td>
              <td />
              <td>{n.descricao}</td>
              <td />
              <td>{botoes(n.evidencias)}</td>
            </tr>
          ))}
          {arestas.map((a, i) => (
            <tr
              key={`aresta-${i}`}
              aria-selected={selecionada === `aresta-${i}`}
              data-selecionada={selecionada === `aresta-${i}` || undefined}
              data-tracejada={a.confianca !== "confirmado" || undefined}
            >
              <td>
                {rotuloDe(a.de)} → {rotuloDe(a.para)}
              </td>
              <td>{a.rotulo}</td>
              <td>{a.descricao}</td>
              <td>
                <span className="badge" data-tone={a.confianca === "confirmado" ? "done" : undefined}>
                  {CONFIANCA[a.confianca] ?? a.confianca}
                </span>
              </td>
              <td>{botoes(a.evidencias)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {trechoAberto && trecho}
    </div>
  );
}
