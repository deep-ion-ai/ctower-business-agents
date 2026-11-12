import { useCallback, useEffect, useRef, useState } from "react";
import {
  mensagemDe,
  type ApiAlteracoes,
  type ApiAnalise,
  type ArquivoAlterado,
  type AvisoAlteracao as Aviso,
  type PedidoAlteracao,
} from "./analise";

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

/**
 * Aviso de alteração na pasta do sistema (spec, seção 4.6; #29): os arquivos que as
 * sessões desta análise alteraram, com o diff, e as opções de desfazer (com
 * confirmação) e de manter. Relê a cada aviso `alteracao` da vigia sobre a análise.
 * Sem alterações, não mostra nada.
 */
export function AvisoAlteracao({
  api,
  vigiar,
  sistema,
  analise,
}: {
  api: ApiAlteracoes;
  vigiar: ApiAnalise["vigiar"];
  sistema: string;
  analise: string;
}) {
  const [aviso, setAviso] = useState<Aviso | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [abertos, setAbertos] = useState<Set<string>>(new Set());
  const [confirmacao, setConfirmacao] = useState<Confirmacao | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const ativo = useRef(true);

  const carregar = useCallback(() => {
    Promise.resolve()
      .then(() => api.alteracoes(sistema, analise))
      .then((a) => {
        if (!ativo.current) return;
        // Sem o comando (ou com resposta vazia), a tela segue sem aviso.
        setAviso(a && Array.isArray(a.arquivos) ? a : null);
      })
      .catch((e: unknown) => ativo.current && setFalha(mensagemDe(e)));
  }, [api, sistema, analise]);

  useEffect(() => {
    ativo.current = true;
    setAviso(null);
    setFalha(null);
    let parar: (() => void) | null = null;
    let desmontado = false;
    vigiar(sistema, (a) => {
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
  }, [vigiar, sistema, analise, carregar]);

  if (!aviso && !falha) return null;
  if (aviso && aviso.arquivos.length === 0 && !falha) return null;

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
        ? api.desfazer(sistema, analise, alvo.map(pedido))
        : api.manter(sistema, analise, alvo.map(pedido));
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

  return (
    <section className="aviso-alteracao" role="alert" aria-label="Aviso de alteração">
      {arquivos.length > 0 && (
        <>
          <h3>A pasta do sistema foi alterada nesta análise</h3>
          <p>
            Durante a conversa,{" "}
            {arquivos.length === 1 ? "um arquivo" : `${arquivos.length} arquivos`} do sistema{" "}
            {arquivos.length === 1 ? "foi alterado" : "foram alterados"}. O produto analisa e não
            altera o código; a decisão sobre o repositório é sua. Confira o diff e desfaça o que não
            deveria ter mudado.
          </p>
          <ul className="arquivos-alterados">
            {arquivos.map((a) => (
              <li key={chave(a)}>
                <span className={`selo alteracao-${a.tipo}`}>{ROTULO_TIPO[a.tipo]}</span>{" "}
                <code>{a.caminho}</code>{" "}
                {a.diff !== null && (
                  <button type="button" onClick={() => alternar(a)}>
                    {abertos.has(chave(a)) ? "Esconder o diff" : "Ver o diff"}
                  </button>
                )}{" "}
                {a.desfazivel && (
                  <button
                    type="button"
                    disabled={ocupado}
                    onClick={() => setConfirmacao({ acao: "desfazer", arquivos: [a] })}
                  >
                    Desfazer
                  </button>
                )}{" "}
                <button
                  type="button"
                  disabled={ocupado || !a.desfazivel}
                  onClick={() => setConfirmacao({ acao: "manter", arquivos: [a] })}
                >
                  Manter
                </button>
                {a.binario && <span className="suave"> (arquivo binário, sem diff)</span>}
                {!a.desfazivel && (
                  <span className="suave"> (sem conteúdo anterior guardado: confira pelo git)</span>
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
          <button type="button" disabled={ocupado} onClick={executar}>
            {confirmacao.acao === "desfazer" ? "Sim, desfazer" : "Sim, manter"}
          </button>{" "}
          <button type="button" disabled={ocupado} onClick={() => setConfirmacao(null)}>
            Cancelar
          </button>
        </div>
      )}
      {aviso?.avisos.map((t) => (
        <p key={t} className="suave">
          {t}
        </p>
      ))}
      {falha && (
        <p className="trecho-erro" role="status">
          {falha}
        </p>
      )}
    </section>
  );
}
