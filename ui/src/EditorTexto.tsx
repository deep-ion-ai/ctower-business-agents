import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useDeferredValue, useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import {
  CONFLITO,
  ESTADO_DO_TEXTO,
  type ApiConfiguracoes,
  type AvisoTexto,
  type EstadoTexto,
  type LeituraTexto,
} from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { inserirNoCursor, irParaLinha } from "./inserirNoCursor";
import { PreviaMarkdown } from "./PreviaMarkdown";

/** Espera depois da última tecla antes de pedir a validação. */
export const ATRASO_VALIDACAO_MS = 400;

type Dialogo = "voltar" | "descartar" | "restaurar" | "conflito" | "fechar";

/**
 * Editor de um texto editável da base (#104), em tela cheia sobre as configurações:
 * `textarea` monoespaçado à esquerda, prévia à direita (abas em janela estreita) e um
 * painel recolhível com as variáveis aceitas e os avisos da validação.
 *
 * Quem usa decide o que fazer ao sair (`aoSair`) e é avisado de cada mudança no
 * arquivo (`aoAlterado`, para a tabela reler o estado) e de haver ou não alterações
 * sem salvar (`aoPendencia`, para a navegação confirmar a troca de aba).
 */
export function EditorTexto({
  api,
  arquivo,
  aoSair,
  aoAlterado,
  aoPendencia,
}: {
  api: ApiConfiguracoes;
  /** Caminho do texto, relativo à base. */
  arquivo: string;
  aoSair: () => void;
  aoAlterado: () => void;
  aoPendencia?: (pendente: boolean) => void;
}) {
  const [leitura, setLeitura] = useState<LeituraTexto | null>(null);
  const [texto, setTexto] = useState("");
  /** O texto da última leitura ou gravação: o que está no arquivo. */
  const [salvo, setSalvo] = useState("");
  const [versao, setVersao] = useState("");
  const [estado, setEstado] = useState<EstadoTexto>("padrao");
  const [avisos, setAvisos] = useState<AvisoTexto[]>([]);
  const [falha, setFalha] = useState<string | null>(null);
  const [recado, setRecado] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [dialogo, setDialogo] = useState<Dialogo | null>(null);
  const [aba, setAba] = useState<"editar" | "previa">("editar");
  const [painel, setPainel] = useState(true);
  const campo = useRef<HTMLTextAreaElement>(null);
  // A prévia de um roteiro de centenas de linhas não segura a digitação.
  const textoDaPrevia = useDeferredValue(texto);
  const pendente = leitura !== null && texto !== salvo;

  const aplicar = useCallback((l: LeituraTexto) => {
    setLeitura(l);
    setTexto(l.texto);
    setSalvo(l.texto);
    setVersao(l.versao);
    setEstado(l.estado);
  }, []);

  useEffect(() => {
    let ativo = true;
    api
      .lerTexto(arquivo)
      .then((l) => {
        if (!ativo) return;
        aplicar(l);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, arquivo, aplicar]);

  useEffect(() => {
    if (leitura) campo.current?.focus();
  }, [leitura?.arquivo]); // eslint-disable-line react-hooks/exhaustive-deps

  // Avisa a navegação de que há (ou não) alterações sem salvar.
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);
  useEffect(() => () => pendencia.current?.(false), []);

  // Validação com debounce (a primeira, logo que o texto chega).
  const primeira = useRef(true);
  useEffect(() => {
    if (!leitura) return;
    let ativo = true;
    const espera = primeira.current ? 0 : ATRASO_VALIDACAO_MS;
    primeira.current = false;
    const t = setTimeout(() => {
      api
        .validarTexto(arquivo, texto)
        .then((a) => ativo && setAvisos(a))
        // A validação é só um auxílio: sem ela, o texto continua editável.
        .catch(() => undefined);
    }, espera);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, arquivo, texto, leitura]);

  const gravar = async (versaoBase?: string): Promise<boolean> => {
    setOcupado(true);
    setFalha(null);
    setRecado(null);
    const enviado = texto;
    try {
      const r = await api.salvarTexto(arquivo, enviado, versaoBase ?? versao);
      setSalvo(enviado);
      setVersao(r.versao);
      setEstado(r.estado);
      setAvisos(r.avisos);
      setRecado("Salvo.");
      setDialogo(null);
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
      aplicar(await api.lerTexto(arquivo));
      setRecado("Texto recarregado do arquivo.");
      setDialogo(null);
      aoAlterado();
    } catch (e) {
      setFalha(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  /** Sobrescrever é salvar de novo com a versão que o arquivo tem agora. */
  const sobrescrever = async () => {
    setOcupado(true);
    try {
      const atual = await api.lerTexto(arquivo);
      await gravar(atual.versao);
    } catch (e) {
      setDialogo(null);
      setFalha(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  const restaurar = async () => {
    setOcupado(true);
    setFalha(null);
    try {
      aplicar(await api.restaurarTexto(arquivo));
      setRecado("Padrão restaurado.");
      setDialogo(null);
      aoAlterado();
    } catch (e) {
      setDialogo(null);
      setFalha(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  const descartar = () => {
    setTexto(salvo);
    setDialogo(null);
    setRecado("Alterações descartadas.");
  };

  const abrirFora = () => {
    setFalha(null);
    api
      .abrirTexto(arquivo)
      .then(async (caminho) => {
        setRecado(
          pendente
            ? `Aberto no editor do computador: ${caminho}. O que você escreveu aqui ainda não foi salvo.`
            : `Aberto no editor do computador: ${caminho}.`,
        );
        aoAlterado();
        // A cópia pode ter sido criada agora: relê para a versão acompanhar.
        if (!pendente) aplicar(await api.lerTexto(arquivo));
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const voltar = () => {
    if (pendente) setDialogo("voltar");
    else aoSair();
  };

  const inserir = (nome: string) => {
    setAba("editar");
    if (campo.current) inserirNoCursor(campo.current, `{{${nome}}}`);
  };

  const linhaDoAviso = (linha: number) => {
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

  // Fechar a janela com alterações pendentes: o navegador pergunta (`beforeunload`)
  // e, no app, o evento de fechamento da janela do Tauri abre a nossa confirmação.
  const semPerguntar = useRef(false);
  useEffect(() => {
    if (!pendente) return;
    const aviso = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", aviso);
    let desligar: (() => void) | undefined;
    let cancelado = false;
    try {
      getCurrentWindow()
        .onCloseRequested((e) => {
          if (semPerguntar.current) return;
          e.preventDefault();
          setDialogo("fechar");
        })
        .then((f) => (cancelado ? f() : (desligar = f)))
        .catch(() => undefined);
    } catch {
      // Fora do Tauri (navegador, testes) não há janela para escutar.
    }
    return () => {
      cancelado = true;
      window.removeEventListener("beforeunload", aviso);
      desligar?.();
    };
  }, [pendente]);

  const fecharJanela = () => {
    semPerguntar.current = true;
    pendencia.current?.(false);
    getCurrentWindow()
      .destroy()
      .catch((e: unknown) => {
        semPerguntar.current = false;
        setDialogo(null);
        setFalha(`Não foi possível fechar a janela: ${mensagemDe(e)}`);
      });
  };

  if (!leitura) {
    return (
      <section className="editor-texto" aria-label="Editor de texto">
        <header className="editor-texto-topo">
          <button className="btn" type="button" onClick={aoSair}>
            Voltar
          </button>
        </header>
        {falha ? (
          <p className="error-text" role="alert">
            Não foi possível abrir o texto: {falha}
          </p>
        ) : (
          <p className="muted">Carregando o texto…</p>
        )}
      </section>
    );
  }

  const aceitos = leitura.variaveis.map((v) => v.nome);
  const semCopia = estado === "padrao";

  return (
    <section className="editor-texto" aria-label={`Editor de ${leitura.rotulo}`}>
      <header className="editor-texto-topo">
        <button className="btn" type="button" onClick={voltar} disabled={ocupado}>
          Voltar
        </button>
        <div className="editor-texto-titulo">
          <h3>{leitura.rotulo}</h3>
          <code>{leitura.arquivo}</code>
        </div>
        <span className="indicador-pendente" data-pendente={pendente}>
          {pendente ? "Alterações não salvas" : ESTADO_DO_TEXTO[estado]}
        </span>
        <div className="editor-texto-acoes">
          <button
            className="btn btn-primary"
            type="button"
            disabled={!pendente || ocupado}
            onClick={() => void gravar()}
            title="Ctrl+S"
          >
            Salvar
          </button>
          <button
            className="btn"
            type="button"
            disabled={!pendente || ocupado}
            onClick={() => setDialogo("descartar")}
          >
            Descartar alterações
          </button>
          <button
            className="btn"
            type="button"
            disabled={ocupado || (semCopia && !pendente)}
            onClick={() => setDialogo("restaurar")}
          >
            Restaurar o padrão
          </button>
          <button className="btn" type="button" disabled={ocupado} onClick={abrirFora}>
            Abrir no editor do computador
          </button>
          <button
            className="btn"
            type="button"
            aria-expanded={painel}
            aria-controls="painel-editor-texto"
            onClick={() => setPainel((v) => !v)}
          >
            {painel ? "Ocultar painel" : `Variáveis e avisos${avisos.length ? ` (${avisos.length})` : ""}`}
          </button>
        </div>
      </header>

      {estado === "ilegivel" && (
        <p className="error-text" role="alert">
          A cópia deste texto na base está ilegível, e o app usa o padrão. Salvar a substitui.
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {recado && (
        <p className="muted" role="status">
          {recado}
        </p>
      )}

      <div className="editor-texto-abas" role="tablist" aria-label="Painéis do editor">
        <button
          type="button"
          role="tab"
          aria-selected={aba === "editar"}
          onClick={() => setAba("editar")}
        >
          Editar
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={aba === "previa"}
          onClick={() => setAba("previa")}
        >
          Prévia
        </button>
      </div>

      <div className="editor-texto-corpo" data-aba={aba}>
        <div className="editor-texto-col" data-coluna="editar">
          <textarea
            ref={campo}
            className="editor-texto-campo"
            aria-label="Texto"
            spellCheck={false}
            value={texto}
            onChange={(e) => {
              setTexto(e.target.value);
              setRecado(null);
            }}
          />
        </div>
        <div className="editor-texto-col" data-coluna="previa" role="region" aria-label="Prévia">
          <PreviaMarkdown texto={textoDaPrevia} aceitos={aceitos} />
        </div>
        {painel && (
          <aside
            className="editor-texto-lateral"
            id="painel-editor-texto"
            aria-label="Variáveis e avisos"
          >
            <h4>Variáveis e marcadores</h4>
            {leitura.variaveis.length === 0 ? (
              <p className="muted">Este texto é livre: não tem variáveis nem marcadores.</p>
            ) : (
              <>
                <p className="muted">Clique para inserir no cursor.</p>
                <ul className="lista-variaveis">
                  {leitura.variaveis.map((v) => (
                    <li key={v.nome}>
                      <button
                        type="button"
                        className="chip-variavel"
                        title={v.descricao || undefined}
                        onClick={() => inserir(v.nome)}
                      >{`{{${v.nome}}}`}</button>
                      {v.descricao && <span className="muted"> {v.descricao}</span>}
                    </li>
                  ))}
                </ul>
              </>
            )}
            <h4>Avisos</h4>
            <div aria-live="polite">
              {avisos.length === 0 ? (
                <p className="muted">Nenhum aviso.</p>
              ) : (
                <ul className="lista-avisos">
                  {avisos.map((a, i) => (
                    <li key={`${i}:${a.variavel}:${a.linha ?? ""}`}>
                      {a.linha !== null && (
                        <button
                          type="button"
                          className="link"
                          onClick={() => linhaDoAviso(a.linha!)}
                        >
                          Linha {a.linha}
                        </button>
                      )}{" "}
                      {a.mensagem}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </aside>
        )}
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
          <p>Você alterou {leitura.rotulo} e não salvou. O que fazer com as alterações?</p>
        </DialogoConfirmacao>
      )}
      {dialogo === "fechar" && (
        <DialogoConfirmacao
          titulo="Fechar com alterações não salvas"
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
                onClick={() => void gravar().then((ok) => ok && fecharJanela())}
              >
                Salvar e fechar
              </button>
              <button className="btn" type="button" onClick={fecharJanela}>
                Descartar e fechar
              </button>
            </>
          }
        >
          <p>Você alterou {leitura.rotulo} e não salvou. Fechar a janela perde as alterações.</p>
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
              <button className="btn" type="button" onClick={descartar}>
                Descartar
              </button>
            </>
          }
        >
          <p>O texto volta ao que está salvo. O que você escreveu desde então se perde.</p>
        </DialogoConfirmacao>
      )}
      {dialogo === "restaurar" && (
        <DialogoConfirmacao
          titulo="Restaurar o padrão"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Cancelar
              </button>
              <button className="btn" type="button" disabled={ocupado} onClick={() => void restaurar()}>
                Restaurar o padrão
              </button>
            </>
          }
        >
          <p>
            {leitura.grupo === "regrasDoProjeto"
              ? "O arquivo volta ao texto padrão das regras do projeto."
              : "A cópia na base é removida e vale o padrão do app."}{" "}
            O texto atual do arquivo e as alterações não salvas se perdem.
          </p>
        </DialogoConfirmacao>
      )}
      {dialogo === "conflito" && (
        <DialogoConfirmacao
          titulo="O arquivo mudou fora do app"
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
                Sobrescrever com o meu texto
              </button>
            </>
          }
        >
          <p>
            {leitura.arquivo} foi alterado depois que você o abriu (por outro editor, talvez). Recarregar
            descarta o que você escreveu aqui; sobrescrever descarta a alteração feita fora do app.
          </p>
        </DialogoConfirmacao>
      )}
    </section>
  );
}
