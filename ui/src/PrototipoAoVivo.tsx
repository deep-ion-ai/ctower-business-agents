import { useEffect, useState } from "react";
import {
  mensagemDe,
  type ElementoTela,
  type RastroTela,
  type TelaPrototipo,
  type TipoElemento,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import { contar } from "./formatacao";
import {
  ESTADOS_TELA,
  aConfirmar,
  apiVisualizadorTauri,
  comEstado,
  referencias,
  type ApiVisualizador,
  type EstadoTela,
} from "./visualizadorPrototipo";

const ROTULO_TIPO: Record<TipoElemento, string> = {
  campo: "campo",
  validacao: "validação",
  mensagem: "mensagem",
  acao: "ação",
};

/** "a confirmar", com a pergunta de que o item depende, quando há. */
function SeloAConfirmar({ rastro }: { rastro: RastroTela }) {
  if (!aConfirmar(rastro)) return null;
  return (
    <span className="badge" data-tone="attention">
      {rastro.pergunta ? `a confirmar: pergunta ${rastro.pergunta}` : "a confirmar"}
    </span>
  );
}

/** As regras, afirmações e critérios de que o item vem; sem nenhum, nada. */
function Origens({ rastro }: { rastro: RastroTela }) {
  const refs = referencias(rastro);
  if (refs.length === 0) return null;
  return (
    <p className="origens">
      Origem:{" "}
      {refs.map((r) => (
        <span key={r.rotulo} className="chip" title={r.texto}>
          {r.rotulo}
        </span>
      ))}
    </p>
  );
}

function ListaElementos({ tela }: { tela: TelaPrototipo }) {
  const rotulo = `TL-${tela.numero}`;
  if (tela.elementos.length === 0) {
    return <p className="muted">Nenhum elemento registrado nesta tela.</p>;
  }
  return (
    <ol className="criterios elementos-tela" aria-label={`Elementos de ${rotulo}`}>
      {tela.elementos.map((e: ElementoTela) => (
        <li key={e.numero} className="criterio" aria-label={`E-${e.numero}: ${e.nome}`}>
          <div className="criterio-cabecalho">
            <strong>E-{e.numero}</strong> {e.nome}
            <span className="chip">{ROTULO_TIPO[e.tipo]}</span>
            <SeloAConfirmar rastro={e} />
          </div>
          {e.descricao && <p className="muted">{e.descricao}</p>}
          <Origens rastro={e} />
        </li>
      ))}
    </ol>
  );
}

/** Pedir ajuste na tela ou retirar o pedido: a decisão vai pela revisão da análise. */
function RevisaoTela({ tela, revisao }: { tela: TelaPrototipo; revisao: Revisao }) {
  const [aberto, setAberto] = useState(false);
  const [pedido, setPedido] = useState("");
  const rotulo = `TL-${tela.numero}`;
  const decidir = (d: Parameters<Revisao["aoDecidir"]>[0]) =>
    revisao
      .aoDecidir(d)
      .then(() => {
        setAberto(false);
        setPedido("");
      })
      .catch(() => undefined);
  if (aberto) {
    return (
      <form
        className="form-quebra"
        aria-label={`Pedir ajuste em ${rotulo}`}
        onSubmit={(e) => {
          e.preventDefault();
          if (!pedido.trim()) return;
          void decidir({ tipo: "tela", numero: tela.numero, decisao: "ajustar", pedido: pedido.trim() });
        }}
      >
        <label>
          O que ajustar
          <textarea
            rows={3}
            required
            value={pedido}
            onChange={(e) => setPedido(e.target.value)}
          />
        </label>
        <div className="acoes">
          <button
            type="submit"
            className="btn btn-primary"
            disabled={revisao.ocupado || !pedido.trim()}
          >
            Pedir o ajuste
          </button>
          <button type="button" className="btn" onClick={() => setAberto(false)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  }
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão de ${rotulo}`}>
      <button
        type="button"
        className="btn"
        disabled={revisao.ocupado}
        onClick={() => {
          setPedido(tela.ajuste_pedido ?? "");
          setAberto(true);
        }}
      >
        {tela.ajuste_pedido ? "Mudar o pedido de ajuste" : "Pedir ajuste"}
      </button>
      {tela.ajuste_pedido && (
        <button
          type="button"
          className="btn"
          disabled={revisao.ocupado}
          onClick={() => void decidir({ tipo: "tela", numero: tela.numero, decisao: "desfazer" })}
        >
          Desfazer o pedido
        </button>
      )}
    </div>
  );
}

/**
 * O protótipo HTML da tela num `iframe` com `sandbox` vazio: sem script, sem
 * formulário, sem navegação para fora e sem a origem do app; o documento ainda
 * traz a própria CSP, que não deixa carregar nada de fora (#385).
 */
function QuadroTela({
  api,
  projeto,
  analise,
  versao,
  tela,
  estado,
}: {
  api: ApiVisualizador;
  projeto: string;
  analise: string;
  versao: number;
  tela: TelaPrototipo;
  estado: EstadoTela | null;
}) {
  const [html, setHtml] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const comHtml = !!tela.html;

  useEffect(() => {
    setHtml(null);
    setFalha(null);
    if (!comHtml) return;
    let ativo = true;
    api
      .htmlDaTela(projeto, analise, tela.numero)
      .then((h) => ativo && setHtml(h))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
    // A versão da análise muda quando o agente grava o HTML de novo.
  }, [api, projeto, analise, tela.numero, comHtml, versao]);

  const rotulo = `TL-${tela.numero}`;
  if (!comHtml) {
    return (
      <p className="aviso-prototipo attention-text" role="status">
        A tela {rotulo} ainda não tem protótipo HTML: o agente grava o HTML depois de
        registrar os elementos.
      </p>
    );
  }
  if (falha) {
    return (
      <p className="error-text" role="alert">
        Não foi possível abrir o protótipo de {rotulo}: {falha}
      </p>
    );
  }
  if (html === null) return <p className="muted">Abrindo o protótipo…</p>;
  return (
    <iframe
      className="prototipo-quadro"
      title={`Protótipo de ${rotulo}: ${tela.nome}`}
      sandbox=""
      referrerPolicy="no-referrer"
      srcDoc={comEstado(html, estado)}
    />
  );
}

/**
 * Protótipo de telas em revisão (fluxo `prototipo`, #382): a lista das telas
 * (TL-n, nome, nova ou alterada, motivo e o que está a confirmar), a tela
 * escolhida num visualizador isolado (sem script e sem rede), com a escolha do
 * estado, e os elementos com a origem de cada um. Na revisão, o analista pede
 * ajuste numa tela; a aprovação é a da análise. Sem `revisao`, só leitura.
 */
export function PrototipoAoVivo({
  projeto,
  analise,
  versao,
  telas,
  origem,
  revisao,
  api = apiVisualizadorTauri,
}: {
  projeto: string;
  analise: string;
  /** Versão da análise: muda quando o agente grava algo, e o HTML é lido de novo. */
  versao: number;
  telas: TelaPrototipo[];
  origem?: string | null;
  revisao?: Revisao;
  api?: ApiVisualizador;
}) {
  const [escolhida, setEscolhida] = useState<number | null>(null);
  const [estado, setEstado] = useState<EstadoTela | null>(null);
  const tela = telas.find((t) => t.numero === escolhida) ?? telas[0];
  const aConfirmarTelas = telas.filter(
    (t) => aConfirmar(t) || t.elementos.some((e) => aConfirmar(e)),
  ).length;
  const comAjuste = telas.filter((t) => t.ajuste_pedido).length;
  const semHtml = telas.filter((t) => !t.html).length;

  return (
    <section aria-label="Protótipo de telas" className="historias-ao-vivo prototipo-ao-vivo">
      <h3>Protótipo de telas</h3>
      <p className="muted">
        {contar(telas.length, "tela", "telas")}
        {aConfirmarTelas > 0 && ` · ${aConfirmarTelas} com itens a confirmar`}
        {semHtml > 0 && ` · ${semHtml} sem protótipo HTML`}
        {comAjuste > 0 && ` · ${contar(comAjuste, "ajuste pedido", "ajustes pedidos")}`}
        {origem && ` · origem ${origem}`}
      </p>
      {telas.length === 0 && <p className="muted">Nenhuma tela registrada ainda.</p>}
      {tela && (
        <div className="prototipo-visualizador">
          <nav aria-label="Telas do protótipo" className="prototipo-lista-telas">
            <ol>
              {telas.map((t) => {
                const atual = t.numero === tela.numero;
                return (
                  <li key={t.numero}>
                    <button
                      type="button"
                      className="prototipo-item-tela"
                      aria-current={atual ? "true" : undefined}
                      onClick={() => setEscolhida(t.numero)}
                    >
                      <span className="painel-id">TL-{t.numero}</span> <strong>{t.nome}</strong>
                      <span className="prototipo-marcas">
                        <span className="chip">{t.mudanca}</span>
                        {aConfirmar(t) && (
                          <span className="badge" data-tone="attention">
                            a confirmar
                          </span>
                        )}
                        {t.ajuste_pedido && (
                          <span className="badge" data-tone="attention">
                            ajuste pedido
                          </span>
                        )}
                        {!t.html && <span className="muted">sem HTML</span>}
                      </span>
                      <span className="muted prototipo-motivo">{t.motivo}</span>
                    </button>
                  </li>
                );
              })}
            </ol>
          </nav>

          <article
            className="prototipo-tela-escolhida"
            aria-label={`TL-${tela.numero}: ${tela.nome}`}
          >
            <header className="historia-cabecalho">
              <span className="painel-id">TL-{tela.numero}</span>
              <strong>{tela.nome}</strong>
              <span className="chip">{tela.mudanca}</span>
              <SeloAConfirmar rastro={tela} />
            </header>
            <p>
              <strong>Motivo:</strong> {tela.motivo}
            </p>
            <Origens rastro={tela} />
            {tela.ajuste_pedido && (
              <p className="ajuste-pedido attention-text">
                <strong>Ajuste pedido:</strong> {tela.ajuste_pedido}
              </p>
            )}
            {revisao && <RevisaoTela key={tela.numero} tela={tela} revisao={revisao} />}

            <div className="prototipo-estados-app" role="group" aria-label="Estado da tela">
              <button
                type="button"
                className="btn btn-sm"
                aria-pressed={estado === null}
                onClick={() => setEstado(null)}
              >
                Todos
              </button>
              {ESTADOS_TELA.map((e) => (
                <button
                  key={e.valor}
                  type="button"
                  className="btn btn-sm"
                  aria-pressed={estado === e.valor}
                  onClick={() => setEstado(e.valor)}
                >
                  {e.rotulo}
                </button>
              ))}
            </div>
            <QuadroTela
              api={api}
              projeto={projeto}
              analise={analise}
              versao={versao}
              tela={tela}
              estado={estado}
            />

            <h4>Elementos</h4>
            <ListaElementos tela={tela} />
          </article>
        </div>
      )}
    </section>
  );
}
