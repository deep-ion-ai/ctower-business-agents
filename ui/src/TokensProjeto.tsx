import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import {
  ROTULO_CATEGORIA_TOKEN,
  type ApiPrototipo,
  type CategoriaToken,
  type PapelToken,
  type TokensVista,
} from "./prototipo";

const CATEGORIAS: CategoriaToken[] = ["cor", "tipografia", "espacamento", "raio"];

const SITUACAO: Record<PapelToken["situacao"], string> = {
  encontrado: "do código",
  informado: "informado por você",
  "a-confirmar": "a confirmar",
};

const AVISO_WIREFRAME =
  "Enquanto a identidade visual não for aprovada, as telas saem em wireframe neutro. O pedido do protótipo não depende dela.";

/**
 * Cartão "Identidade visual" (#379): a revisão dos tokens CSS do projeto (#384) antes
 * de o protótipo usá-los. Mostra o estado (rascunho ou aprovado), extrai do código da
 * interface do projeto, lista os papéis (`--prototipo-*`) com os "a confirmar" em
 * destaque, deixa informar o valor de um papel (ou voltar ao do código) e aprovar.
 * Não bloqueia nada: sem tokens aprovados, as telas saem em wireframe neutro.
 */
export function TokensProjeto({
  api,
  projeto,
  aoMudar,
}: {
  api: ApiPrototipo;
  projeto: string;
  /** Avisa quem usa o cartão quando os tokens carregam ou mudam. */
  aoMudar?: (tokens: TokensVista | null) => void;
}) {
  // `undefined`: carregando; `null`: ainda não extraídos.
  const [tokens, setTokens] = useState<TokensVista | null | undefined>(undefined);
  const [aberto, setAberto] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [editando, setEditando] = useState<string | null>(null);

  const mostrar = useCallback(
    (t: TokensVista | null) => {
      setTokens(t);
      aoMudar?.(t);
    },
    [aoMudar],
  );

  useEffect(() => {
    if (!projeto) return;
    let ativo = true;
    setTokens(undefined);
    api
      .tokens(projeto)
      .then((t) => {
        if (!ativo) return;
        mostrar(t);
        // Em rascunho, a revisão já vem aberta: é o que falta fazer.
        setAberto(t?.estado === "rascunho");
      })
      .catch((e: unknown) => ativo && (setTokens(null), setFalha(mensagemDe(e))));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);

  const executar = (acao: () => Promise<TokensVista>, abrir = false) => {
    setOcupado(true);
    setFalha(null);
    acao()
      .then((t) => {
        mostrar(t);
        setEditando(null);
        if (abrir) setAberto(true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const extrair = () => executar(() => api.extrairTokens(projeto), true);
  const aprovar = () => executar(() => api.aprovarTokens(projeto));
  const informar = (papel: string, valor?: string) =>
    executar(() => api.informarToken(projeto, papel, valor));

  const aprovado = tokens?.estado === "aprovado";

  return (
    <section className="identidade-visual" aria-label="Identidade visual">
      <div className="identidade-visual-cabecalho">
        <h4>Identidade visual</h4>
        {tokens && (
          <span className="badge" data-tone={aprovado ? "done" : "attention"}>
            {aprovado ? "Aprovada" : "Rascunho, aguarda sua revisão"}
          </span>
        )}
      </div>
      {tokens === undefined ? (
        <p className="muted">Carregando os tokens do projeto…</p>
      ) : tokens === null ? (
        <>
          <p className="muted">
            Os tokens CSS (cores, tipografia, espaçamento e raio) ainda não foram extraídos do
            código da interface deste projeto.
          </p>
          <div className="acoes">
            <button type="button" className="btn" disabled={ocupado} onClick={extrair}>
              Extrair do código
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="muted">
            {tokens.papeis.length} papéis, {tokens.tokens.length} tokens achados no código
            {tokens.aConfirmar > 0 && (
              <>
                {" · "}
                <strong className="a-confirmar">{tokens.aConfirmar} a confirmar</strong>
              </>
            )}
            . Arquivo: <span className="mono">{tokens.arquivo}</span>
          </p>
          <div className="acoes">
            <button
              type="button"
              className="btn"
              aria-expanded={aberto}
              onClick={() => setAberto(!aberto)}
            >
              {aberto ? "Esconder os papéis" : "Revisar os papéis"}
            </button>
            <button type="button" className="btn" disabled={ocupado} onClick={extrair}>
              Extrair de novo
            </button>
            <button
              type="button"
              className="btn btn-primary"
              disabled={ocupado || aprovado}
              onClick={aprovar}
            >
              {aprovado ? "Identidade visual aprovada" : "Aprovar a identidade visual"}
            </button>
          </div>
          {aberto &&
            CATEGORIAS.map((c) => {
              const papeis = tokens.papeis.filter((p) => p.categoria === c);
              if (papeis.length === 0) return null;
              return (
                <div key={c} className="papeis-token">
                  <h5>{ROTULO_CATEGORIA_TOKEN[c]}</h5>
                  <ul aria-label={`Papéis: ${ROTULO_CATEGORIA_TOKEN[c]}`}>
                    {papeis.map((p) => (
                      <LinhaPapel
                        key={p.nome}
                        papel={p}
                        editando={editando === p.nome}
                        ocupado={ocupado}
                        aoEditar={() => setEditando(p.nome)}
                        aoCancelar={() => setEditando(null)}
                        aoInformar={(v) => informar(p.nome, v)}
                      />
                    ))}
                  </ul>
                </div>
              );
            })}
        </>
      )}
      {!aprovado && tokens !== undefined && (
        <p className="muted" role="note">
          {AVISO_WIREFRAME}
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </section>
  );
}

function LinhaPapel({
  papel,
  editando,
  ocupado,
  aoEditar,
  aoCancelar,
  aoInformar,
}: {
  papel: PapelToken;
  editando: boolean;
  ocupado: boolean;
  aoEditar: () => void;
  aoCancelar: () => void;
  /** Sem valor, volta ao do código. */
  aoInformar: (valor?: string) => void;
}) {
  const [valor, setValor] = useState(papel.valor ?? "");
  const aConfirmar = papel.situacao === "a-confirmar";
  const propriedade = `--prototipo-${papel.nome}`;
  // Exemplo do valor que o analista digita, não uma cor desta tela.
  const exemplo = papel.categoria === "cor" ? "Ex.: #1f6feb" : "Ex.: 8px"; // estilos:ignorar

  const salvar = (ev: FormEvent) => {
    ev.preventDefault();
    if (valor.trim()) aoInformar(valor.trim());
  };

  return (
    <li data-situacao={papel.situacao}>
      <div className="papel-token-linha">
        {papel.categoria === "cor" && papel.valor && (
          <span
            className="amostra-cor"
            aria-hidden="true"
            style={{ background: papel.valor.startsWith("var(") ? undefined : papel.valor }}
          />
        )}
        <span className="mono">{propriedade}</span>
        <span className="muted">{papel.descricao}</span>
        {aConfirmar ? (
          <span className="badge" data-tone="attention">
            a confirmar
          </span>
        ) : (
          <span>
            <span className="mono">{papel.valor}</span>{" "}
            <span className="muted">({SITUACAO[papel.situacao]})</span>
          </span>
        )}
        {!editando && (
          <span className="papel-token-acoes">
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              aria-label={`${aConfirmar ? "Informar" : "Alterar"} ${propriedade}`}
              onClick={aoEditar}
            >
              {aConfirmar ? "Informar" : "Alterar"}
            </button>
            {papel.situacao === "informado" && (
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                aria-label={`Voltar ${propriedade} ao do código`}
                onClick={() => aoInformar(undefined)}
              >
                Voltar ao do código
              </button>
            )}
          </span>
        )}
      </div>
      {(papel.nota || papel.token) && (
        <p className="muted papel-token-nota">
          {papel.token && (
            <>
              Token do código: <span className="mono">{papel.token}</span>
              {papel.origem && ` (${papel.origem.arquivo}:${papel.origem.linha})`}.{" "}
            </>
          )}
          {papel.nota}
        </p>
      )}
      {editando && (
        // Um formulário dentro do pedido do protótipo não pode ser <form>: fica um grupo.
        <div className="papel-token-edicao" role="group" aria-label={`Valor de ${propriedade}`}>
          <input
            aria-label={`Valor de ${propriedade}`}
            value={valor}
            placeholder={exemplo}
            onChange={(e) => setValor(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") salvar(e);
              if (e.key === "Escape") {
                e.stopPropagation();
                aoCancelar();
              }
            }}
          />
          <button
            type="button"
            className="btn btn-primary"
            disabled={ocupado || !valor.trim()}
            onClick={salvar}
          >
            Salvar
          </button>
          <button type="button" className="btn" onClick={aoCancelar}>
            Cancelar
          </button>
        </div>
      )}
    </li>
  );
}
