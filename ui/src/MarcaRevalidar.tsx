import type { Fundamento, MarcaRevalidacao } from "./analise";
import type { DecisaoRevisao, ItemRequisito } from "./revisao";

/** O item está "a revalidar" (#169). */
export function aRevalidar(m: MarcaRevalidacao): boolean {
  return m.estado === "a-revalidar";
}

/** Marca feita pelo analista (a revalidação não tira). */
export function doAnalista(m: MarcaRevalidacao): boolean {
  return aRevalidar(m) && m.antes_de_revalidar == null;
}

function explicacao(m: MarcaRevalidacao): string {
  if (doAnalista(m)) return "Marcado pelo analista para rever.";
  const fundamentos = m.fundamentos_a_revalidar ?? [];
  return fundamentos.length > 0
    ? `O código citado mudou desde o registro (${fundamentos.join(", ")}).`
    : "O código citado mudou desde o registro.";
}

/** Selo "a revalidar" de um critério, cenário ou tabela; nada sem a marca. */
export function SeloRevalidar({ marca }: { marca: MarcaRevalidacao }) {
  if (!aRevalidar(marca)) return null;
  return (
    <span className="badge" data-tone="attention" title={explicacao(marca)}>
      a revalidar{doAnalista(marca) ? " (analista)" : ""}
    </span>
  );
}

/** Fundamento cuja evidência não confere mais com o código. */
export function fundamentoARevalidar(m: MarcaRevalidacao, f: Fundamento): boolean {
  const rotulo = f.tipo === "afirmacao" ? f.ref : f.id;
  return (m.fundamentos_a_revalidar ?? []).includes(rotulo);
}

/** Chip de um fundamento, em destaque quando a evidência dele não confere mais. */
export function ChipFundamento({
  marca,
  fundamento,
  rotulo,
  texto,
}: {
  marca: MarcaRevalidacao;
  fundamento: Fundamento;
  rotulo: string;
  texto: string;
}) {
  const mudou = fundamentoARevalidar(marca, fundamento);
  return (
    <span
      className="chip"
      data-tone={mudou ? "attention" : undefined}
      title={mudou ? `${texto} (o código citado mudou)` : texto}
    >
      {rotulo}
    </span>
  );
}

/**
 * Botão do analista: marca o item "a revalidar" ou, marcado, tira a marca
 * depois de rever.
 */
export function BotaoRevalidar({
  marca,
  item,
  ocupado,
  aoDecidir,
}: {
  marca: MarcaRevalidacao;
  item: ItemRequisito;
  ocupado: boolean;
  aoDecidir: (d: DecisaoRevisao) => unknown;
}) {
  const marcado = aRevalidar(marca);
  return (
    <button
      type="button"
      className="btn"
      disabled={ocupado}
      title={marcado ? "Revisto: tirar a marca de a revalidar" : "Marcar para rever"}
      onClick={() => aoDecidir({ tipo: "revalidar", item, a_revalidar: !marcado })}
    >
      {marcado ? "Revisto" : "Marcar a revalidar"}
    </button>
  );
}
