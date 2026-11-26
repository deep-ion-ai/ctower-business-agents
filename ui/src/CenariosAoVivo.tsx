import { useState, type ReactNode } from "react";
import { contar } from "./formatacao";
import type {
  Cenario,
  EvidenciaRegistrada,
  Fundamento,
  Gherkin,
  MarcaRevalidacao,
  SituacaoItem,
  TabelaDecisao,
  Tecnica,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import { Cartao, EstadoVazio, Selo, TituloSecao } from "./componentes";
import type { DecisaoRevisao, ItemRequisito } from "./revisao";
import { BotaoRevalidar, ChipFundamento, SeloRevalidar, aRevalidar } from "./MarcaRevalidar";

export const ROTULO_TECNICA: Record<Tecnica, string> = {
  tabela_de_decisao: "tabela de decisão",
  particao_de_equivalencia: "partição de equivalência",
  valor_limite: "valor-limite",
  negativo: "negativo",
  excecao: "exceção",
};

const SITUACAO: Record<SituacaoItem, string> = {
  pendente: "sem decisão",
  aceito: "aceito",
  descartado: "descartado",
};

function rotuloFundamento(f: Fundamento): string {
  return f.tipo === "afirmacao" ? `af. ${f.ref}` : f.id;
}

function textoFundamento(f: Fundamento): string {
  return f.tipo === "afirmacao" ? f.texto : f.titulo;
}

function local(e: EvidenciaRegistrada): string {
  const [a, b] = e.linhas.split("-");
  return `${e.arquivo}:${!b || a === b ? a : e.linhas}`;
}

/** Evidência do limite ou de uma condição; em destaque quando não confere mais (#169). */
function CodigoEvidencia({ e, className }: { e: EvidenciaRegistrada; className?: string }) {
  const mudou = e.estado === "a-revalidar";
  return (
    <code
      className={className}
      data-tone={mudou ? "attention" : undefined}
      title={mudou ? `${e.trecho ?? ""} (o código citado mudou)`.trim() : e.trecho}
    >
      {local(e)}
    </code>
  );
}

function Origens({ fundamentos, marca }: { fundamentos: Fundamento[]; marca: MarcaRevalidacao }) {
  return (
    <p className="origens">
      Origem:{" "}
      {fundamentos.map((f) => (
        <ChipFundamento
          key={rotuloFundamento(f)}
          marca={marca}
          fundamento={f}
          rotulo={rotuloFundamento(f)}
          texto={textoFundamento(f)}
        />
      ))}
    </p>
  );
}

/** Aceitar, descartar ou desfazer: a decisão comum de tabelas e cenários. */
function BotoesSituacao({
  rotulo,
  situacao,
  revisao,
  decidir,
  extra,
  marca,
  item,
}: {
  rotulo: string;
  situacao: SituacaoItem;
  revisao: Revisao;
  decidir: (s: SituacaoItem) => void;
  extra?: ReactNode;
  marca: MarcaRevalidacao;
  item: ItemRequisito;
}) {
  const ocupado = revisao.ocupado;
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão de ${rotulo}`}>
      {situacao !== "aceito" && (
        <button type="button" className="btn" disabled={ocupado} onClick={() => decidir("aceito")}>
          Aceitar
        </button>
      )}
      {extra}
      {situacao !== "descartado" && (
        <button
          type="button"
          className="btn"
          disabled={ocupado}
          onClick={() => decidir("descartado")}
        >
          Descartar
        </button>
      )}
      {situacao !== "pendente" && (
        <button type="button" className="btn" disabled={ocupado} onClick={() => decidir("pendente")}>
          Desfazer
        </button>
      )}
      <BotaoRevalidar
        marca={marca}
        item={item}
        ocupado={ocupado}
        aoDecidir={(d) => revisao.aoDecidir(d).catch(() => undefined)}
      />
    </div>
  );
}

function CartaoTabela({
  tabela,
  cenarios,
  revisao,
}: {
  tabela: TabelaDecisao;
  cenarios: Cenario[];
  revisao?: Revisao;
}) {
  const t = tabela;
  const rotulo = `TD-${t.numero}`;
  const daColuna = (n: number) =>
    cenarios
      .filter((c) => c.situacao !== "descartado" && c.coluna?.tabela === t.numero && c.coluna.coluna === n)
      .map((c) => c.id);
  const impossiveis = t.colunas.filter((c) => c.impossivel);
  return (
    <Cartao className={`tabela-decisao situacao-${t.situacao}`} rotulo={`${rotulo}: ${t.titulo}`}>
      <header className="historia-cabecalho">
        <span className="painel-id">{rotulo}</span>
        <strong>{t.titulo}</strong>
        <Selo tom={t.situacao === "aceito" ? "concluido" : "neutro"}>{SITUACAO[t.situacao]}</Selo>
        <SeloRevalidar marca={t} />
      </header>
      <div className="tabela-rolagem">
        <table aria-label={`Tabela de decisão ${rotulo}`}>
          <thead>
            <tr>
              <th scope="col">Condições e ações</th>
              {t.colunas.map((c) => (
                <th key={c.numero} scope="col">
                  R{c.numero}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {t.condicoes.map((cond, i) => (
              <tr key={`c${i}`}>
                <th scope="row">
                  <strong>C{i + 1}</strong> {cond.texto}
                  {cond.evidencias?.map((e) => (
                    <CodigoEvidencia key={local(e)} e={e} className="evidencia-condicao" />
                  ))}
                </th>
                {t.colunas.map((c) => (
                  <td key={c.numero}>{c.valores[i] === "-" ? "—" : c.valores[i]}</td>
                ))}
              </tr>
            ))}
            {t.acoes.map((acao, i) => (
              <tr key={`a${i}`}>
                <th scope="row">
                  <strong>A{i + 1}</strong> {acao}
                </th>
                {t.colunas.map((c) => (
                  <td key={c.numero}>
                    {c.impossivel ? "·" : (c.acoes ?? []).includes(i + 1) ? "X" : ""}
                  </td>
                ))}
              </tr>
            ))}
            <tr>
              <th scope="row">Cenário</th>
              {t.colunas.map((c) => {
                const lista = daColuna(c.numero);
                return (
                  <td key={c.numero} className={c.impossivel ? "muted" : undefined}>
                    {c.impossivel ? "impossível" : lista.length > 0 ? lista.join(", ") : "—"}
                  </td>
                );
              })}
            </tr>
          </tbody>
        </table>
      </div>
      {impossiveis.length > 0 && (
        <ul className="impossiveis" aria-label={`Combinações impossíveis de ${rotulo}`}>
          {impossiveis.map((c) => (
            <li key={c.numero}>
              <strong>R{c.numero}</strong> impossível: {c.impossivel}
            </li>
          ))}
        </ul>
      )}
      <Origens fundamentos={t.fundamentos} marca={t} />
      {revisao && (
        <BotoesSituacao
          rotulo={rotulo}
          marca={t}
          item={{ tipo: "tabela", numero: t.numero }}
          situacao={t.situacao}
          revisao={revisao}
          decidir={(situacao) =>
            void revisao.aoDecidir({ tipo: "tabela", numero: t.numero, situacao }).catch(() => undefined)
          }
        />
      )}
    </Cartao>
  );
}

/** Tabela dos valores-limite, com o limite lido do código em destaque. */
function TabelaLimites({ cenarios }: { cenarios: Cenario[] }) {
  const comLimite = cenarios.filter((c) => c.limite && c.situacao !== "descartado");
  if (comLimite.length === 0) return null;
  return (
    <section aria-label="Valores-limite" className="valores-limite">
      <h4>Valores-limite e partições</h4>
      <div className="tabela-rolagem">
        <table>
          <thead>
            <tr>
              <th scope="col">Cenário</th>
              <th scope="col">Grandeza</th>
              <th scope="col">Limite no código</th>
              <th scope="col">Valor testado</th>
              <th scope="col">Resultado esperado</th>
              <th scope="col">Evidência</th>
            </tr>
          </thead>
          <tbody>
            {comLimite.map((c) => {
              const l = c.limite!;
              return (
                <tr key={c.id}>
                  <td>{c.id}</td>
                  <td>{l.grandeza}</td>
                  <td>
                    <strong className="limite">{l.valor}</strong>
                    {l.configuracao && (
                      <Selo tom="atencao" titulo={l.configuracao}>
                        a confirmar
                      </Selo>
                    )}
                  </td>
                  <td>{c.valor ?? "—"}</td>
                  <td>{c.entao}</td>
                  <td>
                    {l.evidencias.map((e) => (
                      <CodigoEvidencia key={local(e)} e={e} />
                    ))}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function FormCenario({
  cenario,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  cenario: Cenario;
  ocupado: boolean;
  aoSalvar: (t: Gherkin & { titulo: string }) => void;
  aoCancelar: () => void;
}) {
  const [t, setT] = useState({
    titulo: cenario.titulo,
    dado: cenario.dado,
    quando: cenario.quando,
    entao: cenario.entao,
  });
  const campo = (chave: keyof typeof t, rotulo: string) => (
    <label>
      {rotulo}
      <textarea rows={2} value={t[chave]} onChange={(e) => setT({ ...t, [chave]: e.target.value })} />
    </label>
  );
  return (
    <form
      className="form-criterio"
      aria-label={`Editar ${cenario.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        aoSalvar(t);
      }}
    >
      {campo("titulo", "Título")}
      {campo("dado", "Dado")}
      {campo("quando", "Quando")}
      {campo("entao", "Então")}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function CartaoCenario({ cenario, revisao }: { cenario: Cenario; revisao?: Revisao }) {
  const [editando, setEditando] = useState(false);
  const c = cenario;
  const decidir = (d: DecisaoRevisao) =>
    revisao
      ?.aoDecidir(d)
      .then(() => setEditando(false))
      .catch(() => undefined);
  return (
    <li className={`criterio situacao-${c.situacao}`} aria-label={`${c.id}: ${c.titulo}`}>
      <div className="criterio-cabecalho">
        <strong>{c.id}</strong> {c.titulo}
        <Selo tom="neutro">{ROTULO_TECNICA[c.tecnica]}</Selo>
        {c.coluna && (
          <Selo tom="neutro">
            TD-{c.coluna.tabela}/R{c.coluna.coluna}
          </Selo>
        )}
        {c.historia && <Selo tom="neutro">{c.historia}</Selo>}
        {c.pergunta && <Selo tom="atencao">a confirmar: pergunta {c.pergunta}</Selo>}
        <SeloRevalidar marca={c} />
        {c.situacao !== "pendente" && (
          <Selo tom={c.situacao === "aceito" ? "concluido" : "neutro"}>{SITUACAO[c.situacao]}</Selo>
        )}
        {c.original && <span className="muted editada">(editado pelo analista)</span>}
      </div>
      {c.limite && (
        <p className="limite-cenario">
          Limite: <strong className="limite">{c.limite.valor}</strong> ({c.limite.grandeza}
          {c.limite.configuracao ? `; em ${c.limite.configuracao}, a confirmar` : ", lido do código"})
        </p>
      )}
      {c.valor && <p className="muted">Valor testado: {c.valor}</p>}
      {editando && revisao ? (
        <FormCenario
          cenario={c}
          ocupado={revisao.ocupado}
          aoCancelar={() => setEditando(false)}
          aoSalvar={(t) => decidir({ tipo: "editar-cenario", cenario: c.id, ...t })}
        />
      ) : (
        <dl className="gherkin">
          <dt>Dado</dt>
          <dd>{c.dado}</dd>
          <dt>Quando</dt>
          <dd>{c.quando}</dd>
          <dt>Então</dt>
          <dd>{c.entao}</dd>
        </dl>
      )}
      <Origens fundamentos={c.fundamentos} marca={c} />
      {revisao && !editando && (
        <BotoesSituacao
          rotulo={c.id}
          marca={c}
          item={{ tipo: "cenario", cenario: c.id }}
          situacao={c.situacao}
          revisao={revisao}
          decidir={(situacao) => void decidir({ tipo: "cenario", cenario: c.id, situacao })}
          extra={
            <button
              type="button"
              className="btn"
              disabled={revisao.ocupado}
              onClick={() => setEditando(true)}
            >
              Editar
            </button>
          }
        />
      )}
    </li>
  );
}

/**
 * Cenários de teste em revisão (fluxo `cenarios`, #117): as tabelas de decisão
 * (condições × ações, com as combinações impossíveis justificadas e o cenário de
 * cada coluna), a tabela dos valores-limite com o limite lido do código em
 * destaque e os cenários (CT-…) com a técnica, Dado / Quando / Então e as regras de
 * origem; o limite que vem de configuração aparece "a confirmar". O analista
 * aceita, edita ou descarta cada cenário e aceita ou descarta cada tabela. Sem
 * `revisao` (análise aprovada), só leitura.
 */
export function CenariosAoVivo({
  tabelas,
  cenarios,
  origem,
  revisao,
}: {
  tabelas: TabelaDecisao[];
  cenarios: Cenario[];
  origem?: string | null;
  revisao?: Revisao;
}) {
  const vigentes = cenarios.filter((c) => c.situacao !== "descartado");
  const aConfirmar = vigentes.filter((c) => c.pergunta).length;
  const revalidar =
    vigentes.filter(aRevalidar).length +
    tabelas.filter((t) => t.situacao !== "descartado" && aRevalidar(t)).length;
  return (
    <section aria-label="Cenários de teste" className="historias-ao-vivo cenarios-ao-vivo">
      <TituloSecao nivel={3}>Cenários de teste</TituloSecao>
      <p className="muted">
        {contar(tabelas.length, "tabela", "tabelas")} de decisão · {contar(vigentes.length, "cenário", "cenários")}
        {aConfirmar > 0 && ` · ${aConfirmar} a confirmar`}
        {revalidar > 0 && ` · ${revalidar} a revalidar`}
        {origem && ` · origem ${origem}`}
      </p>
      {tabelas.length === 0 && cenarios.length === 0 && (
        <EstadoVazio>Nenhum cenário registrado ainda.</EstadoVazio>
      )}
      {tabelas.map((t) => (
        <CartaoTabela key={t.numero} tabela={t} cenarios={cenarios} revisao={revisao} />
      ))}
      <TabelaLimites cenarios={cenarios} />
      {cenarios.length > 0 && (
        <ol className="criterios" aria-label="Cenários">
          {cenarios.map((c) => (
            <CartaoCenario key={c.id} cenario={c} revisao={revisao} />
          ))}
        </ol>
      )}
    </section>
  );
}
