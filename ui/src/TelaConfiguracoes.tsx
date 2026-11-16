import { useCallback, useEffect, useState } from "react";
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
import { escolhaEfetiva, useEscolhaLembrada } from "./configSecoes";
import { EditorTexto } from "./EditorTexto";
import { MenuConfiguracoes } from "./MenuConfiguracoes";
import { ModalNovoProjeto } from "./ModalNovoProjeto";
import { usePendenciasProjetos } from "./pendenciasProjetos";
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
}: {
  api?: ApiConfiguracoes;
  /** Os projetos ou a base mudaram: as outras telas devem reler. */
  aoMudarProjetos?: () => void;
  /** O editor de textos tem (ou deixou de ter) alterações sem salvar. */
  aoPendencia?: (pendente: boolean) => void;
}) {
  const [cfg, setCfg] = useState<Configuracoes | null>(null);
  /** Texto aberto no editor embutido (caminho relativo à base). */
  const [editando, setEditando] = useState<string | null>(null);
  const [textos, setTextos] = useState<TextoEditavel[]>([]);
  const [falha, setFalha] = useState<string | null>(null);
  const [cadastrando, setCadastrando] = useState(false);
  const [lembrada, escolher] = useEscolhaLembrada();

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

  const { noMenu, ambienteVerificado } = usePendenciasProjetos(api, cfg?.projetos ?? SEM_PROJETOS);

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

  const escolha = escolhaEfetiva(lembrada, cfg.projetos);
  const projeto = cfg.projetos.find((p) => p.id === escolha.projeto);

  return (
    <>
      {/* Escondida (não desmontada) enquanto o editor está aberto: ao voltar, tudo
          continua onde estava. */}
      <section className="configuracoes page" aria-label="Configurações" hidden={editando !== null}>
        <h2>Configurações</h2>
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
        <div className="config-layout">
          <MenuConfiguracoes
            escolha={escolha}
            projetos={noMenu}
            podeCadastrar={cfg.base !== null}
            aoEscolher={escolher}
            aoNovoProjeto={() => setCadastrando(true)}
          />
          <div className="config-conteudo">
            {escolha.secao === "aparencia" && <ConfigAparencia />}
            {escolha.secao === "assistente" && (
              <ConfigAssistente api={api} cliente={cfg.cliente} aoMudar={carregar} />
            )}
            {/* “Acessos” (#193): quando existir, `SecaoAcessos` entra aqui, com
                `acessos` fora de SECOES_OCULTAS (configSecoes.ts). */}
            {escolha.secao === "pasta" && (
              <>
                <ConfigPastaBase api={api} cfg={cfg} aoMudar={mudou} />
                {/* Provisório: até a seção Acessos existir, é aqui que ficam a pasta dos
                    clones e os tokens globais. */}
                <SecaoClones api={api} cfg={cfg} />
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
                api={api}
                cfg={cfg}
                projeto={projeto}
                textos={textos}
                aoMudar={mudou}
                aoAbrirTexto={carregar}
                aoEditar={setEditando}
                aoAmbiente={ambienteVerificado}
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
              escolher({ secao: "projeto", projeto: p.id });
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
          aoPendencia={aoPendencia}
        />
      )}
    </>
  );
}

const SEM_PROJETOS: Configuracoes["projetos"] = [];
