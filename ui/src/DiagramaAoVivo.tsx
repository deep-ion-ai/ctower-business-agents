import { useId, useState, type ReactNode } from "react";
import type { Revisao } from "./AcoesRevisao";
import type { AchadoDiagrama, Diagrama, Evidencia, Pergunta } from "./analise";
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

const TIPO_ACHADO: Record<AchadoDiagrama["tipo"], string> = {
  inalcancavel: "Estado inalcançável",
  sem_saida: "Estado sem saída",
  sem_condicao: "Transição sem condição",
};

/**
 * Achado do ciclo de vida (#119), com "Virar pergunta": o texto vem preenchido com o
 * achado e o analista o edita antes de registrar; as evidências vêm do elemento.
 * Achado que já virou pergunta mostra o número dela, sem botão.
 */
function ItemAchado({
  achado,
  diagrama,
  pergunta,
  revisao,
  botoes,
}: {
  achado: AchadoDiagrama;
  diagrama: number;
  pergunta?: Pergunta;
  revisao?: Revisao;
  botoes: ReactNode;
}) {
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(achado.texto);
  const id = useId();
  const registrar = () =>
    revisao
      ?.aoDecidir({
        tipo: "virar-pergunta",
        texto: texto.trim(),
        origem: { tipo: "achado-diagrama", diagrama, achado: achado.id },
      })
      .then(() => setEditando(false))
      .catch(() => undefined);
  return (
    <li className="achado-diagrama" data-tipo={achado.tipo} data-testid={`achado-${achado.id}`}>
      <strong>{TIPO_ACHADO[achado.tipo] ?? achado.tipo}:</strong> {achado.texto} {botoes}
      {pergunta ? (
        <span className="badge" data-tone="done">
          virou a pergunta {pergunta.numero}
        </span>
      ) : (
        revisao &&
        (editando ? (
          <form
            className="form-revisao"
            onSubmit={(e) => {
              e.preventDefault();
              if (texto.trim()) registrar();
            }}
          >
            <label htmlFor={id}>Pergunta para o solicitante</label>
            <textarea id={id} value={texto} rows={3} onChange={(e) => setTexto(e.target.value)} />
            <div>
              <button className="btn" type="submit" disabled={revisao.ocupado || !texto.trim()}>
                Registrar pergunta
              </button>{" "}
              <button className="btn" type="button" onClick={() => setEditando(false)}>
                Cancelar
              </button>
            </div>
          </form>
        ) : (
          <button
            type="button"
            className="btn btn-sm"
            disabled={revisao.ocupado}
            onClick={() => setEditando(true)}
          >
            Virar pergunta
          </button>
        ))
      )}
    </li>
  );
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
  achados = [],
  perguntas = [],
  revisao,
}: {
  diagrama: Diagrama;
  /** Código Mermaid gerado pelo app; sem ele (resposta antiga), só a tabela. */
  mermaid?: string;
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia) => void;
  /** Trecho da evidência aberta, mostrado abaixo da tabela quando `trechoAberto`. */
  trecho?: ReactNode;
  trechoAberto: boolean;
  /** Achados do ciclo de vida calculados pelo app (#119). */
  achados?: AchadoDiagrama[];
  /** Perguntas da análise, para marcar o achado que já virou pergunta. */
  perguntas?: Pergunta[];
  /** Revisão aberta: habilita "Virar pergunta". */
  revisao?: Revisao;
}) {
  const [selecionada, setSelecionada] = useState<Linha | null>(null);
  const nomes = NOMES[diagrama.tipo] ?? NOMES.fluxo;
  const arestas = diagrama.arestas ?? [];
  const rotuloDe = (c: string) => diagrama.nos.find((n) => n.chave === c)?.rotulo ?? c;
  // Condição e efeitos (#119) só ganham coluna quando alguma transição os tem.
  const comGuarda = arestas.some((a) => a.condicao || a.efeitos);
  const perguntaDo = (achado: AchadoDiagrama) =>
    perguntas.find(
      (p) =>
        p.origem?.tipo === "achado-diagrama" &&
        p.origem.diagrama === diagrama.numero &&
        p.origem.achado === achado.id,
    );

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
            {comGuarda && <th scope="col">Condição</th>}
            {comGuarda && <th scope="col">Efeitos</th>}
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
              {comGuarda && <td />}
              {comGuarda && <td />}
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
              {comGuarda && <td>{a.condicao}</td>}
              {comGuarda && <td>{a.efeitos}</td>}
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
      {achados.length > 0 && (
        <div className="achados-diagrama" aria-label={`Achados de ${diagrama.titulo}`}>
          <p>
            <strong>Achados do ciclo de vida</strong>{" "}
            <span className="muted">· calculados pelo app a partir das transições registradas</span>
          </p>
          <ul>
            {achados.map((achado) => (
              <ItemAchado
                key={achado.id}
                achado={achado}
                diagrama={diagrama.numero}
                pergunta={perguntaDo(achado)}
                revisao={revisao}
                botoes={botoes(achado.evidencias)}
              />
            ))}
          </ul>
        </div>
      )}
      {trechoAberto && trecho}
    </div>
  );
}
