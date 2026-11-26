import { useDeferredValue, useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, Selo } from "./componentes";
import { CONFLITO } from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { irParaLinha } from "./inserirNoCursor";
import {
  DESTINO,
  recadoDaMudanca,
  type ApiWorkflows,
  type Destino,
  type LeituraWorkflow,
  type LinhaDiferenca,
  type Validacao,
} from "./workflows";

/** Espera depois da última tecla antes de pedir a validação. */
export const ATRASO_VALIDACAO_WORKFLOW_MS = 400;

type Dialogo = "voltar" | "descartar" | "restaurar" | "conflito";
export type PainelWorkflow = "fases" | "diferenca";

/**
 * Editor de um workflow das análises (#372), em tela cheia sobre as configurações,
 * no molde do editor de textos (#104/#106): o script em JavaScript à esquerda, à
 * direita as fases declaradas em `meta.phases` ou a diferença com o padrão atual do
 * app, e ao lado os erros e avisos da validação (feita em Rust a cada pausa na
 * digitação e de novo ao salvar). Um script com erro não é salvo: a tela mostra a
 * linha e a mensagem. Sem `destino`, mostra o padrão do app, só para leitura.
 */
export function EditorWorkflow({
  api,
  tipo,
  destino,
  painelInicial = "fases",
  aoSair,
  aoAlterado,
  aoPendencia,
}: {
  api: ApiWorkflows;
  tipo: string;
  /** `null`: o padrão do app, só leitura. */
  destino: Destino | null;
  painelInicial?: PainelWorkflow;
  aoSair: () => void;
  /** A cópia mudou (salva, restaurada): a lista deve reler. */
  aoAlterado: () => void;
  aoPendencia?: (pendente: boolean) => void;
}) {
  const [leitura, setLeitura] = useState<LeituraWorkflow | null>(null);
  const [texto, setTexto] = useState("");
  const [salvo, setSalvo] = useState("");
  const [validacao, setValidacao] = useState<Validacao>({ problemas: [], fases: [] });
  const [diferenca, setDiferenca] = useState<LinhaDiferenca[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [recado, setRecado] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [dialogo, setDialogo] = useState<Dialogo | null>(null);
  const [aba, setAba] = useState<"editar" | "previa">("editar");
  const [painel, setPainel] = useState<PainelWorkflow>(painelInicial);
  const campo = useRef<HTMLTextAreaElement>(null);
  const textoAdiado = useDeferredValue(texto);
  const somenteLeitura = destino === null || (leitura !== null && !leitura.existe);
  const pendente = leitura !== null && !somenteLeitura && texto !== salvo;

  const aplicar = (l: LeituraWorkflow) => {
    setLeitura(l);
    setTexto(l.texto);
    setSalvo(l.texto);
    setValidacao(l.validacao);
  };

  useEffect(() => {
    let ativo = true;
    api
      .ler(tipo, destino)
      .then((l) => {
        if (!ativo) return;
        aplicar(l);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, tipo, destino]);

  // Avisa a navegação de que há (ou não) alterações sem salvar.
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);
  useEffect(() => () => pendencia.current?.(false), []);

  // Validação com debounce, só depois de mudar o texto (a leitura já traz a primeira).
  useEffect(() => {
    if (!leitura || texto === leitura.texto) return;
    let ativo = true;
    const t = setTimeout(() => {
      api
        .validar(tipo, texto)
        .then((v) => ativo && setValidacao(v))
        // A validação é só um auxílio enquanto se digita: ao salvar, ela é refeita.
        .catch(() => undefined);
    }, ATRASO_VALIDACAO_WORKFLOW_MS);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, tipo, texto, leitura]);

  // A diferença com o padrão, quando o painel está aberto.
  useEffect(() => {
    if (!leitura || painel !== "diferenca") return;
    let ativo = true;
    api
      .diferenca(tipo, textoAdiado)
      .then((d) => ativo && setDiferenca(d))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, tipo, textoAdiado, painel, leitura]);

  // Fechar a janela com alterações pendentes: o navegador pergunta.
  useEffect(() => {
    if (!pendente) return;
    const aviso = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", aviso);
    return () => window.removeEventListener("beforeunload", aviso);
  }, [pendente]);

  const gravar = async (padraoRevisto = false, versao?: string): Promise<boolean> => {
    if (!leitura || destino === null) return false;
    setOcupado(true);
    setFalha(null);
    setRecado(null);
    const enviado = texto;
    try {
      const r = await api.salvar(tipo, destino, enviado, versao ?? leitura.versao, padraoRevisto);
      if (!r.salvo) {
        setValidacao(r.leitura.validacao);
        setDialogo(null);
        setFalha("O script tem erros e não foi salvo. Corrija as linhas indicadas em Erros e avisos.");
        return false;
      }
      setLeitura(r.leitura);
      setSalvo(enviado);
      setValidacao(r.leitura.validacao);
      setDialogo(null);
      setRecado(recadoDaMudanca(r, "Salvo. A próxima análise usa esta versão."));
      if (r.avisos.length) setFalha(r.avisos.join(" "));
      aoAlterado();
      return true;
    } catch (e) {
      const m = mensagemDe(e);
      if (m.startsWith(CONFLITO)) setDialogo("conflito");
      else {
        setDialogo(null);
        setFalha(m);
      }
      return false;
    } finally {
      setOcupado(false);
    }
  };

  const recarregar = async () => {
    setOcupado(true);
    setFalha(null);
    try {
      aplicar(await api.ler(tipo, destino));
      setRecado("Cópia recarregada do arquivo.");
      setDialogo(null);
    } catch (e) {
      setFalha(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  /** Sobrescrever é salvar de novo com a versão que o arquivo tem agora. */
  const sobrescrever = async () => {
    try {
      const atual = await api.ler(tipo, destino);
      await gravar(false, atual.versao);
    } catch (e) {
      setDialogo(null);
      setFalha(mensagemDe(e));
    }
  };

  const restaurar = async () => {
    if (destino === null) return;
    setOcupado(true);
    setFalha(null);
    try {
      const r = await api.restaurar(tipo, destino);
      aplicar(r.leitura);
      setRecado(recadoDaMudanca(r, "Cópia apagada: vale a versão de baixo."));
      setDialogo(null);
      aoAlterado();
    } catch (e) {
      setDialogo(null);
      setFalha(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  const voltar = () => {
    if (pendente) setDialogo("voltar");
    else aoSair();
  };

  const irPara = (linha: number) => {
    setAba("editar");
    if (campo.current) irParaLinha(campo.current, linha);
  };

  // Ctrl+S (Cmd+S no macOS) salva, onde estiver o foco.
  const aoSalvar = useRef<() => void>(() => undefined);
  aoSalvar.current = () => {
    if (pendente && !ocupado && dialogo === null) void gravar();
  };
  useEffect(() => {
    const tecla = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        aoSalvar.current();
      }
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, []);

  if (!leitura) {
    return (
      <section className="editor-texto" aria-label="Editor de workflow">
        <header className="editor-texto-topo">
          <button className="btn" type="button" onClick={aoSair}>
            Voltar
          </button>
        </header>
        {falha ? (
          <Alerta tom="erro" titulo="Não foi possível abrir o workflow:">
            {falha}
          </Alerta>
        ) : (
          <p className="muted">Carregando o workflow…</p>
        )}
      </section>
    );
  }

  const titulo =
    destino === null ? `${leitura.rotulo}: padrão do app` : `${leitura.rotulo}: ${DESTINO[destino].toLowerCase()}`;
  const erros = validacao.problemas.filter((p) => p.erro).length;

  return (
    <section className="editor-texto editor-workflow" aria-label={`Editor do workflow ${leitura.nome}`}>
      <header className="editor-texto-topo">
        <button className="btn" type="button" onClick={voltar} disabled={ocupado}>
          Voltar
        </button>
        <div className="editor-texto-titulo">
          <h3>{titulo}</h3>
          <code>{leitura.arquivo ?? `${leitura.nome} (embutido no app)`}</code>
        </div>
        <span className="indicador-pendente" data-pendente={pendente}>
          {somenteLeitura
            ? "só leitura"
            : pendente
              ? "Alterações não salvas"
              : erros
                ? "com erros: não é usada"
                : "salva"}
        </span>
        {!somenteLeitura && (
          <BarraAcoes
            rotulo="Ações do workflow"
            alinhamento="fim"
            principal={{
              texto: "Salvar",
              aoEscolher: () => void gravar(),
              desabilitado: !pendente || ocupado,
              dica: "Ctrl+S",
            }}
            secundarias={[
              {
                texto: "Descartar alterações",
                aoEscolher: () => setDialogo("descartar"),
                desabilitado: !pendente || ocupado,
              },
              { texto: "Restaurar o padrão", aoEscolher: () => setDialogo("restaurar"), desabilitado: ocupado },
            ]}
          />
        )}
      </header>

      {destino !== null && !leitura.existe && (
        <p className="muted" role="status">
          Ainda não há {DESTINO[destino].toLowerCase()}. Duplique o workflow em Configurações ›
          Workflows para editá-lo.
        </p>
      )}
      {leitura.existe && leitura.padraoMudou && (
        <Alerta
          tom="atencao"
          titulo="O padrão do app mudou desde a sua cópia."
          acoes={
            <>
              <button
                className="btn btn-sm"
                type="button"
                onClick={() => {
                  setPainel("diferenca");
                  setAba("previa");
                }}
              >
                Ver a diferença
              </button>
              <button
                className="btn btn-sm"
                type="button"
                disabled={ocupado}
                onClick={() => void gravar(true)}
              >
                {pendente ? "Salvar e marcar como revista" : "Já revi: marcar como revista"}
              </button>
            </>
          }
        >
          A cópia continua valendo como está: o app não mescla. Veja a diferença e traga para a
          cópia o que quiser.
        </Alerta>
      )}
      {falha && <Alerta tom="erro" titulo={falha} />}
      {recado && (
        <p className="muted" role="status">
          {recado}
        </p>
      )}

      <div className="editor-texto-abas" role="tablist" aria-label="Painéis do editor">
        <button type="button" role="tab" aria-selected={aba === "editar"} onClick={() => setAba("editar")}>
          Script
        </button>
        <button type="button" role="tab" aria-selected={aba === "previa"} onClick={() => setAba("previa")}>
          Fases e diferença
        </button>
      </div>

      <div className="editor-texto-corpo" data-aba={aba}>
        <div className="editor-texto-col" data-coluna="editar">
          <textarea
            ref={campo}
            className="editor-texto-campo"
            aria-label="Script do workflow (JavaScript)"
            spellCheck={false}
            readOnly={somenteLeitura}
            value={texto}
            onChange={(e) => {
              setTexto(e.target.value);
              setRecado(null);
            }}
          />
        </div>
        <div className="editor-texto-col" data-coluna="previa" role="region" aria-label="Prévia">
          <div className="editor-workflow-paineis" role="tablist" aria-label="Prévia do workflow">
            <button
              type="button"
              role="tab"
              className="btn btn-sm"
              aria-selected={painel === "fases"}
              onClick={() => setPainel("fases")}
            >
              Fases
            </button>
            <button
              type="button"
              role="tab"
              className="btn btn-sm"
              aria-selected={painel === "diferenca"}
              onClick={() => setPainel("diferenca")}
            >
              Diferença com o padrão
            </button>
          </div>
          {painel === "fases" ? (
            <PreviaFases validacao={validacao} />
          ) : (
            <PreviaDiferenca linhas={diferenca} />
          )}
        </div>
        <aside className="editor-texto-lateral" aria-label="Erros e avisos">
          <h4>Erros e avisos</h4>
          <div aria-live="polite">
            {validacao.problemas.length === 0 ? (
              <p className="muted">Nenhum problema.</p>
            ) : (
              <ul className="lista-avisos">
                {validacao.problemas.map((p, i) => (
                  <li key={`${i}:${p.linha ?? ""}`} data-erro={p.erro}>
                    <Selo tom={p.erro ? "erro" : "atencao"}>{p.erro ? "Erro" : "Aviso"}</Selo>{" "}
                    {p.linha !== null && (
                      <button type="button" className="link" onClick={() => irPara(p.linha!)}>
                        Linha {p.linha}
                      </button>
                    )}{" "}
                    {p.mensagem}
                  </li>
                ))}
              </ul>
            )}
          </div>
          <p className="muted">
            O app confere a estrutura (o <code>meta</code>, as fases, o que quebra a retomada). Um erro de
            sintaxe JavaScript só aparece ao rodar: aí a análise segue sem workflow, com aviso.
          </p>
        </aside>
      </div>

      {dialogo === "voltar" && (
        <DialogoConfirmacao
          titulo="Alterações não salvas"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Continuar editando
              </button>
              <button
                className="btn"
                type="button"
                disabled={ocupado}
                onClick={() => void gravar().then((ok) => ok && aoSair())}
              >
                Salvar e voltar
              </button>
              <button className="btn" type="button" onClick={aoSair}>
                Descartar e voltar
              </button>
            </>
          }
        >
          <p>Você alterou o workflow e não salvou. O que fazer com as alterações?</p>
        </DialogoConfirmacao>
      )}
      {dialogo === "descartar" && (
        <DialogoConfirmacao
          titulo="Descartar as alterações"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Continuar editando
              </button>
              <button
                className="btn"
                type="button"
                onClick={() => {
                  setTexto(salvo);
                  setValidacao(leitura.validacao);
                  setDialogo(null);
                  setRecado("Alterações descartadas.");
                }}
              >
                Descartar
              </button>
            </>
          }
        >
          <p>O script volta ao que está salvo. O que você escreveu desde então se perde.</p>
        </DialogoConfirmacao>
      )}
      {dialogo === "restaurar" && destino !== null && (
        <DialogoConfirmacao
          titulo="Restaurar o padrão"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Cancelar
              </button>
              <button className="btn btn-danger" type="button" disabled={ocupado} onClick={() => void restaurar()}>
                Apagar a cópia
              </button>
            </>
          }
        >
          <p>
            A {DESTINO[destino].toLowerCase()} é apagada e as análises voltam a usar a versão de baixo
            {destino === "projeto" ? " (a cópia para todos os projetos, se houver, ou o padrão do app)" : " (o padrão do app)"}.
            O script atual e as alterações não salvas se perdem.
          </p>
        </DialogoConfirmacao>
      )}
      {dialogo === "conflito" && (
        <DialogoConfirmacao
          titulo="A cópia mudou fora do app"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Continuar editando
              </button>
              <button className="btn" type="button" disabled={ocupado} onClick={() => void recarregar()}>
                Recarregar o arquivo
              </button>
              <button className="btn" type="button" disabled={ocupado} onClick={() => void sobrescrever()}>
                Sobrescrever com o meu script
              </button>
            </>
          }
        >
          <p>
            {leitura.arquivo} foi alterado depois que você o abriu. Recarregar descarta o que você
            escreveu aqui; sobrescrever descarta a alteração feita fora do app.
          </p>
        </DialogoConfirmacao>
      )}
    </section>
  );
}

function PreviaFases({ validacao }: { validacao: Validacao }) {
  if (validacao.fases.length === 0) {
    return (
      <p className="muted">
        Sem fases para mostrar: o <code>meta.phases</code> está vazio ou não pôde ser lido (veja os
        erros).
      </p>
    );
  }
  return (
    <ol className="fases-workflow" aria-label="Fases do workflow">
      {validacao.fases.map((f) => (
        <li key={f.titulo}>
          <strong>{f.titulo}</strong>
          {f.detalhe && <span className="muted"> — {f.detalhe}</span>}
        </li>
      ))}
    </ol>
  );
}

function PreviaDiferenca({ linhas }: { linhas: LinhaDiferenca[] | null }) {
  if (linhas === null) return <p className="muted">Comparando com o padrão…</p>;
  if (linhas.every((l) => l.tipo === "igual")) {
    return <p className="muted">Igual ao padrão atual do app.</p>;
  }
  return (
    <pre className="diferenca-workflow" aria-label="Diferença com o padrão">
      {linhas.map((l, i) => (
        <div key={i} data-tipo={l.tipo}>
          {l.tipo === "removida" ? "- " : l.tipo === "acrescentada" ? "+ " : "  "}
          {l.texto}
        </div>
      ))}
    </pre>
  );
}
