import { useCallback, useEffect, useRef, useState } from "react";
import { BotaoPainel } from "./BotaoPainel";
import {
  mensagemDe,
  type ApiAlteracoes,
  type ApiAnalise,
  type ArquivoAlterado,
  type AvisoAlteracao as Aviso,
  type PedidoAlteracao,
} from "./analise";

/** Tom do selo: a modificação pede o olhar do analista (âmbar); o texto diz o tipo. */
const TOM_TIPO: Record<ArquivoAlterado["tipo"], string> = {
  criado: "done",
  modificado: "attention",
  removido: "error",
};

const ROTULO_TIPO: Record<ArquivoAlterado["tipo"], string> = {
  criado: "criado",
  modificado: "modificado",
  removido: "removido",
};

function chave(a: ArquivoAlterado): string {
  return `${a.sessao}|${a.pasta}|${a.caminho}`;
}

function pedido(a: ArquivoAlterado): PedidoAlteracao {
  return { sessao: a.sessao, pasta: a.pasta, caminho: a.caminho, hashAtual: a.hashAtual };
}

type Confirmacao = { acao: "desfazer" | "manter"; arquivos: ArquivoAlterado[] };

/** Nota fixa do painel: o que o aviso não cobre (#130). */
export const NOTA_CODIGO_FONTE =
  "As pastas de código-fonte dos repositórios cadastrados não entram neste aviso.";

/**
 * Aviso de alteração na pasta do projeto (spec, seção 4.6; #29, #130): um ícone
 * âmbar na barra, com o número de arquivos que as sessões desta análise alteraram.
 * O ícone abre o painel com a lista, o diff e as opções de desfazer (com
 * confirmação) e de manter. Relê a cada aviso `alteracao` da vigia sobre a análise.
 * Sem alterações, não mostra nada. As pastas de código-fonte já ficam de fora no
 * retrato (`cba_session::alteracoes`).
 */
export function AvisoAlteracao({
  api,
  vigiar,
  projeto,
  analise,
  aberto: abertoFora,
  aoAlternar,
  aoFechar,
}: {
  api: ApiAlteracoes;
  vigiar: ApiAnalise["vigiar"];
  projeto: string;
  analise: string;
  /** Painel aberto, quando a barra controla (um painel por vez). */
  aberto?: boolean;
  aoAlternar?: () => void;
  aoFechar?: () => void;
}) {
  const [abertoLocal, setAbertoLocal] = useState(false);
  const controlado = abertoFora !== undefined && aoAlternar && aoFechar;
  const aberto = controlado ? abertoFora : abertoLocal;
  const alternarPainel = controlado ? aoAlternar : () => setAbertoLocal((v) => !v);
  const fecharLocal = useCallback(() => setAbertoLocal(false), []);
  const fechar = controlado ? aoFechar : fecharLocal;
  const [aviso, setAviso] = useState<Aviso | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [abertos, setAbertos] = useState<Set<string>>(new Set());
  const [confirmacao, setConfirmacao] = useState<Confirmacao | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const ativo = useRef(true);

  const carregar = useCallback(() => {
    Promise.resolve()
      .then(() => api.alteracoes(projeto, analise))
      .then((a) => {
        if (!ativo.current) return;
        // Sem o comando (ou com resposta vazia), a tela segue sem aviso.
        setAviso(a && Array.isArray(a.arquivos) ? a : null);
      })
      .catch((e: unknown) => ativo.current && setFalha(mensagemDe(e)));
  }, [api, projeto, analise]);

  useEffect(() => {
    ativo.current = true;
    setAviso(null);
    setFalha(null);
    let parar: (() => void) | null = null;
    let desmontado = false;
    vigiar(projeto, (a) => {
      if (a.tipo === "reinicio" || (a.analise === analise && a.mudanca === "alteracao")) {
        carregar();
      }
    })
      .then((f) => {
        if (desmontado) f();
        else parar = f;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!desmontado) carregar();
      });
    return () => {
      desmontado = true;
      ativo.current = false;
      parar?.();
    };
  }, [vigiar, projeto, analise, carregar]);

  const vazio = (!aviso || aviso.arquivos.length === 0) && !falha;

  // Sem alterações (desfeitas ou mantidas), o painel fecha junto com o ícone.
  useEffect(() => {
    if (vazio && aberto) fechar();
  }, [vazio, aberto, fechar]);

  if (vazio) return null;

  const arquivos = aviso?.arquivos ?? [];
  const desfaziveis = arquivos.filter((a) => a.desfazivel);

  const alternar = (a: ArquivoAlterado) =>
    setAbertos((atual) => {
      const novo = new Set(atual);
      const k = chave(a);
      if (novo.has(k)) novo.delete(k);
      else novo.add(k);
      return novo;
    });

  const executar = () => {
    if (!confirmacao) return;
    const { acao, arquivos: alvo } = confirmacao;
    setOcupado(true);
    setFalha(null);
    const feito =
      acao === "desfazer"
        ? api.desfazer(projeto, analise, alvo.map(pedido))
        : api.manter(projeto, analise, alvo.map(pedido));
    feito
      .then(() => setConfirmacao(null))
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => {
        setOcupado(false);
        carregar();
      });
  };

  const nomes = (lista: ArquivoAlterado[]) =>
    lista.length === 1 ? lista[0].caminho : `${lista.length} arquivos`;

  const n = arquivos.length;
  const rotulo =
    n === 0
      ? "Falha no aviso de alteração"
      : n === 1
        ? "1 arquivo alterado nesta análise"
        : `${n} arquivos alterados nesta análise`;

  return (
    <BotaoPainel
      aberto={aberto}
      aoAlternar={alternarPainel}
      aoFechar={fechar}
      rotulo={rotulo}
      titulo="A pasta do projeto foi alterada"
      classeBotao="btn-icone btn-alteracao"
      conteudo={
        <>
          <IconePastaAlterada />
          <span className="contador" aria-hidden="true">
            {n === 0 ? "!" : n}
          </span>
        </>
      }
      rotuloPainel="Aviso de alteração"
      classePainel="aviso-alteracao"
    >
      {arquivos.length > 0 && (
        <>
          <h3>A pasta do projeto foi alterada nesta análise</h3>
          <p>
            Durante a conversa,{" "}
            {arquivos.length === 1 ? "um arquivo" : `${arquivos.length} arquivos`} do projeto{" "}
            {arquivos.length === 1 ? "foi alterado" : "foram alterados"}. O produto analisa e não
            altera o projeto; a decisão sobre essas pastas é sua. Confira o diff e desfaça o que não
            deveria ter mudado.
          </p>
          <ul className="arquivos-alterados">
            {arquivos.map((a) => (
              <li key={chave(a)}>
                <span className="badge" data-tone={TOM_TIPO[a.tipo]}>
                  {ROTULO_TIPO[a.tipo]}
                </span>{" "}
                <code>{a.caminho}</code>{" "}
                {a.diff !== null && (
                  <button className="btn" type="button" onClick={() => alternar(a)}>
                    {abertos.has(chave(a)) ? "Esconder o diff" : "Ver o diff"}
                  </button>
                )}{" "}
                {a.desfazivel && (
                  <button
                    className="btn"
                    type="button"
                    disabled={ocupado}
                    onClick={() => setConfirmacao({ acao: "desfazer", arquivos: [a] })}
                  >
                    Desfazer
                  </button>
                )}{" "}
                <button
                  className="btn"
                  type="button"
                  disabled={ocupado || !a.desfazivel}
                  onClick={() => setConfirmacao({ acao: "manter", arquivos: [a] })}
                >
                  Manter
                </button>
                {a.binario && <span className="muted"> (arquivo binário, sem diff)</span>}
                {!a.desfazivel && (
                  <span className="muted"> (sem conteúdo anterior guardado: confira pelo git)</span>
                )}
                {a.diff !== null && abertos.has(chave(a)) && (
                  <pre className="diff" aria-label={`Diff de ${a.caminho}`}>
                    {a.diff}
                  </pre>
                )}
              </li>
            ))}
          </ul>
          {desfaziveis.length > 1 && (
            <button
              className="btn"
              type="button"
              disabled={ocupado}
              onClick={() => setConfirmacao({ acao: "desfazer", arquivos: desfaziveis })}
            >
              Desfazer todas
            </button>
          )}
        </>
      )}
      {confirmacao && (
        <div className="confirmar-alteracao" role="dialog" aria-label="Confirmar">
          <p>
            {confirmacao.acao === "desfazer"
              ? `Desfazer ${nomes(confirmacao.arquivos)}? O conteúdo volta ao do início da sessão e o que a sessão escreveu se perde.`
              : `Manter ${nomes(confirmacao.arquivos)}? A alteração sai deste aviso e fica no repositório, sob sua responsabilidade.`}
          </p>
          <button className="btn" type="button" disabled={ocupado} onClick={executar}>
            {confirmacao.acao === "desfazer" ? "Sim, desfazer" : "Sim, manter"}
          </button>{" "}
          <button
            className="btn"
            type="button"
            disabled={ocupado}
            onClick={() => setConfirmacao(null)}
          >
            Cancelar
          </button>
        </div>
      )}
      {aviso?.avisos.map((t) => (
        <p key={t} className="muted">
          {t}
        </p>
      ))}
      {falha && (
        <p className="error-text" role="status">
          {falha}
        </p>
      )}
      <p className="nota-alteracao muted">{NOTA_CODIGO_FONTE}</p>
    </BotaoPainel>
  );
}

function IconePastaAlterada() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      <path d="M12 11v3" />
      <path d="M12 17h.01" />
    </svg>
  );
}
