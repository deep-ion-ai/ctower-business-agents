import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDeferredValue, useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { AjudaMarkdown } from "./AjudaMarkdown";
import { BarraFormatacao } from "./BarraFormatacao";
import { CONFLITO_DOCUMENTO, rotuloTipo, type DocumentoAberto } from "./conhecimento";
import { CorpoMarkdown } from "./DocumentoBase";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { formatoDoAtalho, type Formato } from "./formatacao";
import { aplicarFormatoNoCampo } from "./inserirNoCursor";

type Dialogo = "voltar" | "descartar" | "conflito" | "fechar";

/**
 * Editor de um documento da base (#220): o corpo em Markdown à esquerda (com a barra
 * de formatação do {@link EditorTexto}), a prévia à direita (abas em janela estreita),
 * salvar, descartar e o aviso "Alterações não salvas". O frontmatter não aparece nem
 * muda: o app só regrava o corpo.
 *
 * Quem usa recebe o documento relido a cada gravação (`aoSalvo`, com a prévia e o
 * índice já atualizados), decide o que fazer ao sair (`aoSair`) e é avisado de haver ou
 * não alterações sem salvar (`aoPendencia`, para a navegação confirmar a troca de aba).
 */
export function EditorDocumento({
  documento,
  salvar,
  reler,
  aoSalvo,
  aoSair,
  aoPendencia,
}: {
  documento: DocumentoAberto;
  /** Grava o corpo; `versao` é a do arquivo quando foi lido. Devolve o documento relido. */
  salvar: (markdown: string, versao: string) => Promise<DocumentoAberto>;
  /** Lê o documento como está no arquivo agora (para sobrescrever depois de um conflito). */
  reler: () => Promise<DocumentoAberto>;
  aoSalvo: (documento: DocumentoAberto) => void;
  aoSair: () => void;
  aoPendencia?: (pendente: boolean) => void;
}) {
  const [texto, setTexto] = useState(documento.markdown);
  /** O que está no arquivo: o da abertura ou o da última gravação. */
  const [salvo, setSalvo] = useState(documento.markdown);
  const [versao, setVersao] = useState(documento.versao);
  const [falha, setFalha] = useState<string | null>(null);
  const [recado, setRecado] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [dialogo, setDialogo] = useState<Dialogo | null>(null);
  const [aba, setAba] = useState<"editar" | "previa">("editar");
  const [ajuda, setAjuda] = useState(false);
  const campo = useRef<HTMLTextAreaElement>(null);
  const botaoAjuda = useRef<HTMLButtonElement>(null);
  const textoDaPrevia = useDeferredValue(texto);
  const pendente = texto !== salvo;

  useEffect(() => campo.current?.focus(), []);

  // Avisa a navegação de que há (ou não) alterações sem salvar.
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);
  useEffect(() => () => pendencia.current?.(false), []);

  const gravar = async (versaoBase?: string): Promise<boolean> => {
    setOcupado(true);
    setFalha(null);
    setRecado(null);
    const enviado = texto;
    try {
      const novo = await salvar(enviado, versaoBase ?? versao);
      setSalvo(enviado);
      setVersao(novo.versao);
      setRecado("Salvo. A prévia e a busca já mostram o texto novo.");
      setDialogo(null);
      aoSalvo(novo);
      return true;
    } catch (e) {
      const m = mensagemDe(e);
      if (m.startsWith(CONFLITO_DOCUMENTO)) setDialogo("conflito");
      else {
        setDialogo(null);
        setFalha(m);
      }
      return false;
    } finally {
      setOcupado(false);
    }
  };

  /** Sobrescrever é salvar de novo com a versão que o arquivo tem agora. */
  const sobrescrever = async () => {
    setOcupado(true);
    try {
      const atual = await reler();
      await gravar(atual.versao);
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

  const voltar = () => {
    if (pendente) setDialogo("voltar");
    else aoSair();
  };

  const formatar = (formato: Formato) => {
    if (campo.current) aplicarFormatoNoCampo(campo.current, formato);
  };

  // Ctrl+S (Cmd+S no macOS) salva, onde estiver o foco.
  const aoSalvar = useRef<() => void>(() => undefined);
  aoSalvar.current = () => {
    if (pendente && !ocupado && dialogo === null && !ajuda) void gravar();
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

  return (
    <section className="editor-texto editor-documento" aria-label={`Editor de ${documento.titulo}`}>
      <header className="editor-texto-topo">
        <button className="btn" type="button" onClick={voltar} disabled={ocupado}>
          Voltar ao documento
        </button>
        <div className="editor-texto-titulo">
          <h3>{documento.titulo}</h3>
          <span className="muted">
            {rotuloTipo(documento.tipo)} · <code>{documento.caminho}</code>
          </span>
        </div>
        <span className="indicador-pendente" data-pendente={pendente}>
          {pendente ? "Alterações não salvas" : "Sem alterações"}
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
        </div>
      </header>

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

      <div className="editor-texto-corpo editor-documento-corpo" data-aba={aba}>
        <div className="editor-texto-col" data-coluna="editar">
          <BarraFormatacao
            aoFormatar={formatar}
            aoAjuda={() => setAjuda(true)}
            botaoAjuda={botaoAjuda}
          />
          <textarea
            ref={campo}
            className="editor-texto-campo"
            aria-label="Texto"
            spellCheck={false}
            value={texto}
            onKeyDown={(e) => {
              const f = formatoDoAtalho(e);
              if (!f) return;
              e.preventDefault();
              formatar(f);
            }}
            onChange={(e) => {
              setTexto(e.target.value);
              setRecado(null);
            }}
          />
        </div>
        <div className="editor-texto-col" data-coluna="previa" role="region" aria-label="Prévia">
          <CorpoMarkdown markdown={textoDaPrevia} caminho={documento.caminho} />
        </div>
      </div>

      {ajuda && <AjudaMarkdown aoFechar={() => setAjuda(false)} retorno={botaoAjuda} />}
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
          <p>Você alterou {documento.titulo} e não salvou. O que fazer com as alterações?</p>
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
          <p>Você alterou {documento.titulo} e não salvou. Fechar a janela perde as alterações.</p>
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
      {dialogo === "conflito" && (
        <DialogoConfirmacao
          titulo="O arquivo mudou fora do app"
          aoCancelar={() => setDialogo(null)}
          acoes={
            <>
              <button className="btn btn-primary" type="button" autoFocus onClick={() => setDialogo(null)}>
                Continuar editando
              </button>
              <button className="btn" type="button" disabled={ocupado} onClick={aoSair}>
                Descartar o meu texto e reabrir
              </button>
              <button className="btn" type="button" disabled={ocupado} onClick={() => void sobrescrever()}>
                Sobrescrever com o meu texto
              </button>
            </>
          }
        >
          <p>
            {documento.caminho} foi alterado depois que você o abriu (por outro editor, talvez).
            Reabrir descarta o que você escreveu aqui; sobrescrever descarta a alteração feita fora
            do app.
          </p>
        </DialogoConfirmacao>
      )}
    </section>
  );
}
