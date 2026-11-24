import { useCallback, useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { ConfigAparencia } from "./ConfigAparencia";
import { ConfigAssistente } from "./ConfigAssistente";
import { ConfigModelos } from "./ConfigModelos";
import { ConfigPastaBase } from "./ConfigPastaBase";
import { ConfigProjeto } from "./ConfigProjeto";
import {
  apiConfiguracoesTauri,
  type ApiConfiguracoes,
  type Configuracoes,
  type TextoEditavel,
} from "./configuracoes";
import { escolhaEfetiva, useEscolhaLembrada, type Escolha } from "./configSecoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { EditorTexto } from "./EditorTexto";
import { MenuConfiguracoes } from "./MenuConfiguracoes";
import { ModalNovoProjeto } from "./ModalNovoProjeto";
import { useEstadoProjetos } from "./pendenciasProjetos";
import { SecaoAcessos } from "./SecaoAcessos";
import { SecaoClones } from "./SecaoClones";
import "./configuracoes.css";

/**
 * Configurações (spec, seção 10; #191): menu lateral com App (aparência, assistente de
 * IA, acessos), Base de conhecimento (pasta, modelos e prompts) e Projetos (um item por
 * projeto, mais “+ Novo projeto”, que abre o assistente de cadastro). O conteúdo da
 * direita mostra só a seção escolhida, lembrada ao voltar para a tela. Cada seção é um
 * componente próprio (`Config*`); aqui ficam a leitura das configurações, o editor de
 * textos embutido e a escolha da seção.
 */
export function TelaConfiguracoes({
  api = apiConfiguracoesTauri,
  aoMudarProjetos,
  aoPendencia,
  projetoFixo,
  aoVoltar,
  listarProjetos = true,
  aoConfigurarProjeto,
  aoIrParaConfiguracoes,
}: {
  api?: ApiConfiguracoes;
  /** Os projetos ou a base mudaram: as outras telas devem reler. */
  aoMudarProjetos?: () => void;
  /** O editor de textos tem (ou deixou de ter) alterações sem salvar. */
  aoPendencia?: (pendente: boolean) => void;
  /**
   * "Configurar projeto" (#319): mostra só a página deste projeto, sem o menu. Abre
   * pelo cabeçalho da Visão geral.
   */
  projetoFixo?: string;
  /** Com {@link projetoFixo}: volta à Visão geral. */
  aoVoltar?: () => void;
  /**
   * `false` tira do menu a lista de projetos (fica só "+ Novo projeto"): cada projeto
   * se configura pela Visão geral (#319).
   */
  listarProjetos?: boolean;
  /** Abre "Configurar projeto" de um projeto (sem a lista no menu, ou ao cadastrar). */
  aoConfigurarProjeto?: (projeto: string) => void;
  /** Com {@link projetoFixo}: leva às configurações do app, na seção já escolhida. */
  aoIrParaConfiguracoes?: () => void;
}) {
  const [cfg, setCfg] = useState<Configuracoes | null>(null);
  /** Texto aberto no editor embutido (caminho relativo à base). */
  const [editando, setEditando] = useState<string | null>(null);
  const [textos, setTextos] = useState<TextoEditavel[]>([]);
  const [falha, setFalha] = useState<string | null>(null);
  const [cadastrando, setCadastrando] = useState(false);
  const [lembrada, escolherAgora] = useEscolhaLembrada();
  /** Escolha à espera da confirmação de sair sem salvar “O que é analisado”. */
  const [saida, setSaida] = useState<Escolha | null>(null);
  // Alterações sem salvar: no editor de textos e na página do projeto (#192).
  const pendencias = useRef({ editor: false, projeto: false });
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  const avisarPendencia = useCallback((qual: "editor" | "projeto", p: boolean) => {
    pendencias.current[qual] = p;
    pendencia.current?.(pendencias.current.editor || pendencias.current.projeto);
  }, []);
  const pendenciaDoEditor = useCallback(
    (p: boolean) => avisarPendencia("editor", p),
    [avisarPendencia],
  );
  const pendenciaDoProjeto = useCallback(
    (p: boolean) => avisarPendencia("projeto", p),
    [avisarPendencia],
  );
  const escolher = useCallback(
    (e: Escolha) => {
      if (pendencias.current.projeto) setSaida(e);
      else escolherAgora(e);
    },
    [escolherAgora],
  );

  const carregar = useCallback(() => {
    api
      .configuracoes()
      .then((c) => {
        setCfg(c);
        setFalha(null);
        if (c.base) return api.textos().then(setTextos);
        setTextos([]);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api]);

  useEffect(carregar, [carregar]);

  const mudou = () => {
    carregar();
    aoMudarProjetos?.();
  };

  const { noMenu, estadoDe, reverificarAmbiente, conferirProntidao, garantirProntidao } =
    useEstadoProjetos(api, cfg?.projetos ?? SEM_PROJETOS);
  const prontidao = useCallback(
    (id: string, forcar?: boolean) => (forcar ? void conferirProntidao(id) : garantirProntidao(id)),
    [conferirProntidao, garantirProntidao],
  );
  const irParaAcessos = useCallback(() => {
    if (projetoFixo !== undefined && aoIrParaConfiguracoes) {
      escolherAgora({ secao: "acessos", projeto: null });
      aoIrParaConfiguracoes();
    } else escolher({ secao: "acessos", projeto: null });
  }, [escolher, escolherAgora, projetoFixo, aoIrParaConfiguracoes]);
  /** Abre a página de um projeto: no menu, ou em "Configurar projeto" sem a lista. */
  const abrirProjeto = (id: string) => {
    if ((!listarProjetos || projetoFixo !== undefined) && aoConfigurarProjeto) aoConfigurarProjeto(id);
    else escolher({ secao: "projeto", projeto: id });
  };

  if (!cfg) {
    return (
      <section className="configuracoes page" aria-label="Configurações">
        {falha ? (
          <p className="error-text" role="alert">
            Não foi possível ler as configurações: {falha}
          </p>
        ) : (
          <p className="muted">Carregando as configurações…</p>
        )}
      </section>
    );
  }

  const fixo = projetoFixo !== undefined;
  const escolha: Escolha = fixo
    ? { secao: "projeto", projeto: projetoFixo }
    : escolhaEfetiva(lembrada, listarProjetos ? cfg.projetos : SEM_PROJETOS);
  const projeto = cfg.projetos.find((p) => p.id === escolha.projeto);

  return (
    <>
      {/* Escondida (não desmontada) enquanto o editor está aberto: ao voltar, tudo
          continua onde estava. */}
      <section
        className="configuracoes page"
        aria-label={fixo ? "Configurar projeto" : "Configurações"}
        hidden={editando !== null}
      >
        {fixo ? (
          <div className="cab-secao">
            <h2>Configurar projeto</h2>
            {aoVoltar && (
              <button type="button" className="link" onClick={aoVoltar}>
                ← Visão geral
              </button>
            )}
          </div>
        ) : (
          <h2>Configurações</h2>
        )}
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
        {fixo && !projeto && (
          <p className="muted">Este projeto não está mais cadastrado.</p>
        )}
        <div className={fixo ? "config-layout config-layout-fixo" : "config-layout"}>
          {!fixo && (
            <MenuConfiguracoes
              escolha={escolha}
              projetos={noMenu}
              listarProjetos={listarProjetos}
              podeCadastrar={cfg.base !== null}
              aoEscolher={escolher}
              aoNovoProjeto={() => setCadastrando(true)}
            />
          )}
          <div className="config-conteudo">
            {escolha.secao === "aparencia" && <ConfigAparencia />}
            {escolha.secao === "assistente" && (
              <ConfigAssistente api={api} cliente={cfg.cliente} aoMudar={carregar} />
            )}
            {escolha.secao === "acessos" && (
              <SecaoAcessos
                api={api}
                cfg={cfg}
                aoAbrirProjeto={abrirProjeto}
              />
            )}
            {escolha.secao === "pasta" && (
              <>
                <ConfigPastaBase api={api} cfg={cfg} aoMudar={mudou} />
                <SecaoClones api={api} />
              </>
            )}
            {escolha.secao === "modelos" && (
              <ConfigModelos
                api={api}
                temBase={cfg.base !== null}
                textos={textos}
                aoAbrir={carregar}
                aoEditar={setEditando}
                aoEscolherBase={() => escolher({ secao: "pasta", projeto: null })}
              />
            )}
            {escolha.secao === "projeto" && projeto && (
              <ConfigProjeto
                key={projeto.id}
                api={api}
                cfg={cfg}
                projeto={projeto}
                textos={textos}
                estado={estadoDe(projeto.id)}
                aoMudar={mudou}
                aoAbrirTexto={carregar}
                aoEditar={setEditando}
                aoReverificarAmbiente={reverificarAmbiente}
                aoConferirProntidao={prontidao}
                aoIrParaAcessos={irParaAcessos}
                aoPendencia={pendenciaDoProjeto}
              />
            )}
          </div>
        </div>
        {cadastrando && (
          <ModalNovoProjeto
            api={api}
            aoFechar={() => setCadastrando(false)}
            aoConcluir={(p) => {
              // A página do projeto novo mostra o preparo do ambiente do Claude.
              setCadastrando(false);
              abrirProjeto(p.id);
              mudou();
            }}
          />
        )}
      </section>
      {editando !== null && (
        <EditorTexto
          key={editando}
          api={api}
          arquivo={editando}
          aoSair={() => {
            setEditando(null);
            carregar();
          }}
          aoAlterado={carregar}
          aoPendencia={pendenciaDoEditor}
        />
      )}
      {saida && (
        <DialogoConfirmacao
          titulo="Sair sem salvar?"
          aoCancelar={() => setSaida(null)}
          acoes={
            <>
              <button className="btn" type="button" autoFocus onClick={() => setSaida(null)}>
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                onClick={() => {
                  avisarPendencia("projeto", false);
                  escolherAgora(saida);
                  setSaida(null);
                }}
              >
                Descartar as alterações
              </button>
            </>
          }
        >
          <p>As alterações em “O que é analisado” ainda não foram salvas e serão perdidas.</p>
        </DialogoConfirmacao>
      )}
    </>
  );
}

const SEM_PROJETOS: Configuracoes["projetos"] = [];
