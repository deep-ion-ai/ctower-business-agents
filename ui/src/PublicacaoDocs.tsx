import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import type {
  ApiPublicacao,
  MudancaPrevista,
  PreviaPublicacao,
  Publicacao,
  TipoMudanca,
} from "./publicacao";

interface Props {
  api: ApiPublicacao;
  projeto: string;
  /** Abre o link do PR/MR no navegador. */
  abrirUrl: (url: string) => void | Promise<void>;
}

/** Quantos arquivos a prévia lista por tipo de mudança. */
const MAX_POR_TIPO = 20;

const ROTULO: Record<TipoMudanca, string> = {
  novo: "Novos",
  alterado: "Alterados",
  removido: "Removidos",
};

const RESULTADO: Record<Publicacao["publicado"]["situacao"], string> = {
  aberto: "Publicado: o PR/MR foi aberto.",
  atualizado: "Publicado: o PR/MR aberto recebeu as mudanças.",
  sem_mudancas: "Nada mudou: o repositório já tem esta versão da documentação.",
};

function ListaMudancas({ tipo, itens }: { tipo: TipoMudanca; itens: MudancaPrevista[] }) {
  if (itens.length === 0) return null;
  return (
    <div className="publicacao-grupo">
      <h4>
        {ROTULO[tipo]} ({itens.length})
      </h4>
      <ul aria-label={ROTULO[tipo]}>
        {itens.slice(0, MAX_POR_TIPO).map((m) => (
          <li key={m.caminho}>
            <code>{m.caminho}</code>
          </li>
        ))}
        {itens.length > MAX_POR_TIPO && <li className="muted">e mais {itens.length - MAX_POR_TIPO}</li>}
      </ul>
    </div>
  );
}

/**
 * Publicação dos documentos aprovados da base no destino da documentação do projeto
 * (#276): mostra o que muda na branch base (novos, alterados e removidos), pede a
 * conferência de segredos e dados pessoais e publica numa branch própria com PR/MR,
 * devolvendo o link.
 */
export function PublicacaoDocs({ api, projeto, abrirUrl }: Props) {
  const [previa, setPrevia] = useState<PreviaPublicacao | null>(null);
  const [carregando, setCarregando] = useState(true);
  const [conferido, setConferido] = useState(false);
  const [publicando, setPublicando] = useState(false);
  const [resultado, setResultado] = useState<Publicacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  const carregar = useCallback(async () => {
    setErro(null);
    setResultado(null);
    setConferido(false);
    setCarregando(true);
    try {
      setPrevia(await api.previa(projeto));
    } catch (e) {
      setPrevia(null);
      setErro(mensagemDe(e));
    } finally {
      setCarregando(false);
    }
  }, [api, projeto]);

  useEffect(() => {
    void carregar();
  }, [carregar]);

  const publicar = async () => {
    setErro(null);
    setPublicando(true);
    try {
      const r = await api.publicar(projeto);
      setResultado(r);
      setPrevia(r.previa);
    } catch (e) {
      setErro(mensagemDe(e));
    } finally {
      setPublicando(false);
    }
  };

  const doTipo = (tipo: TipoMudanca) => previa?.mudancas.filter((m) => m.tipo === tipo) ?? [];
  const link = resultado?.publicado.link;

  return (
    <section className="publicacao" aria-label="Publicação da documentação">
      <h3>Publicar a documentação</h3>
      <p className="muted">
        Os documentos aprovados do projeto vão para o repositório numa branch própria, com um
        PR/MR para revisão. Nada é gravado direto na branch base.
      </p>
      {carregando && <p role="status">Conferindo o que muda no repositório…</p>}
      {previa && !carregando && (
        <>
          <dl className="publicacao-destino">
            <dt>Destino</dt>
            <dd>{previa.destino}</dd>
            <dt>Branch</dt>
            <dd>
              <code>{previa.branch}</code> → <code>{previa.base}</code>
            </dd>
            <dt>Aprovados</dt>
            <dd>{previa.aprovados} documentos</dd>
          </dl>
          {previa.nada_a_publicar ? (
            <p role="status">Nada a publicar: o repositório já tem os documentos aprovados.</p>
          ) : (
            <>
              {previa.mudancas.length === 0 ? (
                <p>Nada muda na branch base.</p>
              ) : (
                <div aria-label="O que muda">
                  <ListaMudancas tipo="novo" itens={doTipo("novo")} />
                  <ListaMudancas tipo="alterado" itens={doTipo("alterado")} />
                  <ListaMudancas tipo="removido" itens={doTipo("removido")} />
                </div>
              )}
              {previa.pendente_na_branch && (
                <p className="muted">
                  A branch <code>{previa.branch}</code> já tem uma publicação ainda não mesclada:
                  ela é atualizada com esta versão.
                </p>
              )}
              {!resultado && (
                <>
                  <div className="attention-text" role="note">
                    <p>
                      Os documentos vão como estão na base. Antes de publicar, confira se não há
                      segredos (senhas, tokens, chaves, endereços internos) nem dados pessoais
                      (nomes, CPF, e-mails, telefones) que não podem ir para o repositório.
                    </p>
                    <label>
                      <input
                        type="checkbox"
                        checked={conferido}
                        onChange={(e) => setConferido(e.target.checked)}
                      />{" "}
                      Conferi: não há segredos nem dados pessoais nos documentos
                    </label>
                  </div>
                  <button
                    className="btn btn-primary"
                    type="button"
                    disabled={!conferido || publicando}
                    onClick={publicar}
                  >
                    {publicando ? "Publicando…" : "Publicar"}
                  </button>
                </>
              )}
            </>
          )}
        </>
      )}
      {resultado && (
        <div role="status">
          <p>{RESULTADO[resultado.publicado.situacao]}</p>
          {link && (
            <p>
              <a
                href={link}
                onClick={(e) => {
                  e.preventDefault();
                  void abrirUrl(link);
                }}
              >
                Abrir o PR/MR
              </a>{" "}
              <span className="muted">{link}</span>
            </p>
          )}
        </div>
      )}
      {erro && (
        <>
          <p className="error-text" role="alert">
            {erro}
          </p>
          <button className="btn" type="button" disabled={carregando} onClick={carregar}>
            Tentar de novo
          </button>
        </>
      )}
    </section>
  );
}
