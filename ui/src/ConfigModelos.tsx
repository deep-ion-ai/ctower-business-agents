import { useState } from "react";
import { mensagemDe } from "./analise";
import {
  Alerta,
  BarraAcoes,
  Cartao,
  EstadoVazio,
  ItemLista,
  ListaItens,
  MenuAcoes,
  Secao,
  Selo,
  type TomSelo,
} from "./componentes";
import {
  ESTADO_DO_TEXTO,
  type ApiConfiguracoes,
  type GrupoTexto,
  type TextoEditavel,
} from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { IconeReticencias } from "./icones";

const GRUPOS: { grupo: GrupoTexto; titulo: string; descricao: string }[] = [
  {
    grupo: "template",
    titulo: "Modelos dos documentos",
    descricao: "A estrutura de cada tipo de documento montado pelo app.",
  },
  {
    grupo: "roteiro",
    titulo: "Roteiros dos fluxos",
    descricao: "As instruções que o agente recebe ao iniciar cada tipo de análise.",
  },
  {
    grupo: "prompt",
    titulo: "Prompts prontos",
    descricao: "Os pedidos dos botões da tela da análise.",
  },
  {
    grupo: "exportacao",
    titulo: "Exportação em HTML",
    descricao:
      "O template e a identidade visual (cores, fontes, tamanhos) do documento exportado em HTML.",
  },
];

/** Para que serve cada texto, por grupo e rótulo do catálogo; sem entrada, vale a do grupo. */
const DESCRICAO_DO_TEXTO: Partial<Record<GrupoTexto, Record<string, string>>> = {
  template: {
    "Análise de impacto": "O documento com o que muda, quem é afetado e os riscos.",
    "Pergunta ao sistema": "O documento que registra uma pergunta e a resposta com as fontes.",
    "Documento negocial da funcionalidade": "A funcionalidade explicada em linguagem de negócio.",
    "Caso de uso": "O passo a passo de uma interação, com os fluxos alternativos.",
    "Regra de negócio": "Uma regra, com a origem no código e o que ainda precisa de validação.",
    "Termo do glossário": "A definição de um termo do negócio.",
    "Sugestão de alteração": "A proposta de mudança num documento já aprovado.",
    Projeto: "A página de apresentação do projeto na base.",
    "Memória da área": "O que o app lembra de cada área do projeto entre as conversas.",
    Demanda: "A demanda trazida do Jira, GitHub ou GitLab.",
    Pessoa: "Quem participa das reuniões e das decisões.",
    "Ata de reunião": "A ata, com decisões, regras, dúvidas e próximos passos.",
    "Análise de reunião": "A leitura da reunião contra o que o código faz.",
    "Parecer de revisão": "O parecer sobre um requisito revisado.",
    "Histórias e critérios de aceite": "As histórias de usuário e os critérios de aceite.",
    "Cenários de teste": "Os cenários de teste derivados dos critérios.",
  },
  roteiro: {
    "Perguntar ao sistema": "Como o agente responde a uma pergunta sobre o sistema.",
    "Explicar funcionalidade": "Como o agente explica uma funcionalidade ao analista.",
    "Analisar impacto": "Como o agente levanta o impacto de uma mudança.",
    "Analisar reunião": "Como o agente cruza a ata de uma reunião com o código.",
    "Revisar requisito": "Como o agente revisa um requisito antes do desenvolvimento.",
    "Histórias e critérios": "Como o agente escreve histórias e critérios de aceite.",
    "Cenários de teste": "Como o agente propõe os cenários de teste.",
  },
  exportacao: {
    "Template HTML da exportação": "A estrutura da página: cabeçalho, documento e rodapé.",
    "Tokens CSS da exportação": "Cores, fontes, tamanhos e margens de impressão do documento.",
    "Estilo adicional da exportação": "Regras CSS extras, para ir além dos tokens. Opcional.",
  },
  prompt: {
    Aprofundar: "Pede ao agente mais detalhe sobre o ponto em foco.",
    "Resumir para o solicitante": "Pede um resumo na linguagem de quem pediu a demanda.",
    "Resumir para o cliente": "Pede um resumo próprio para o cliente.",
    "Listar dúvidas para enviar": "Pede a lista de dúvidas que ainda faltam esclarecer.",
    "Gerar histórias e critérios": "Pede as histórias e os critérios de aceite da análise.",
    "Gerar cenários de teste": "Pede os cenários de teste da análise.",
    "Montar o ciclo de vida": "Pede o ciclo de vida da entidade em foco.",
    "Conferir evidências": "Pede que o agente confira as evidências citadas.",
    Retomar: "Pede ao agente que retome de onde a análise parou.",
  },
};

function descricaoDe(t: TextoEditavel): string | undefined {
  return DESCRICAO_DO_TEXTO[t.grupo]?.[t.rotulo];
}

/**
 * Modelos e prompts (#191): os textos que o agente usa, em grupos com a descrição de
 * cada item e o estado (“padrão do app”, “editado por você”, “cópia sem mudanças”).
 * O botão Editar abre o editor embutido; abrir no editor do computador e voltar ao
 * padrão ficam no menu “⋯”. Sem base escolhida, a seção diz por que está vazia.
 */
export function ConfigModelos({
  api,
  temBase,
  textos,
  aoAbrir,
  aoEditar,
  aoEscolherBase,
}: {
  api: ApiConfiguracoes;
  temBase: boolean;
  textos: TextoEditavel[];
  /** Algo mudou nos arquivos (cópia criada, padrão restaurado): ler de novo. */
  aoAbrir: () => void;
  aoEditar: (arquivo: string) => void;
  /** Leva à seção “Pasta da base”. */
  aoEscolherBase: () => void;
}) {
  const [soEditados, setSoEditados] = useState(false);
  const [aberto, setAberto] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [restaurando, setRestaurando] = useState<TextoEditavel | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const abrirNoComputador = (t: TextoEditavel) => {
    setFalha(null);
    api
      .abrirTexto(t.arquivo)
      .then((c) => {
        setAberto(c);
        aoAbrir();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const restaurar = (t: TextoEditavel) => {
    setOcupado(true);
    setFalha(null);
    api
      .restaurarTexto(t.arquivo)
      .then(() => {
        setRestaurando(null);
        setAberto(null);
        aoAbrir();
      })
      .catch((e: unknown) => {
        setRestaurando(null);
        setFalha(mensagemDe(e));
      })
      .finally(() => setOcupado(false));
  };

  return (
    <Secao titulo="Modelos, roteiros e prompts">
      <p className="muted">
        Textos que o agente usa. Ao editar, o app guarda a sua versão na base (
        <code>_templates/</code>) e passa a usá-la no lugar do padrão.
      </p>

      {!temBase ? (
        <EstadoVazio acao={{ texto: "Ir para a pasta da base", aoEscolher: aoEscolherBase }}>
          Sem a pasta da base, não há onde guardar as suas versões dos textos. Escolha a pasta da
          base para ver e editar os modelos.
        </EstadoVazio>
      ) : (
        <>
          <div className="config-filtros" role="group" aria-label="Filtrar textos">
            <button
              type="button"
              className="btn btn-sm"
              aria-pressed={soEditados}
              onClick={() => setSoEditados((v) => !v)}
            >
              Só os editados
            </button>
          </div>
          {GRUPOS.map((g) => {
            const itens = textos.filter(
              (t) => t.grupo === g.grupo && (!soEditados || t.estado === "editado"),
            );
            if (soEditados && itens.length === 0) return null;
            return (
              <Cartao key={g.grupo} rotulo={g.titulo}>
                <div className="config-cartao-topo">
                  <h3>{g.titulo}</h3>
                  <span className="muted">{g.descricao}</span>
                </div>
                <ListaItens>
                  {itens.map((t) => (
                    <ItemLista
                      key={t.arquivo}
                      meta={<Selo tom={tomDoEstado(t)}>{ESTADO_DO_TEXTO[t.estado]}</Selo>}
                      acoes={
                        <BarraAcoes rotulo={`Ações de ${t.rotulo}`} alinhamento="fim">
                          <button
                            className="btn btn-sm"
                            type="button"
                            aria-label={`Editar ${t.rotulo}`}
                            onClick={() => aoEditar(t.arquivo)}
                          >
                            Editar
                          </button>
                          <MenuAcoes
                            rotulo={`Mais ações para ${t.rotulo}`}
                            icone={<IconeReticencias />}
                            itens={[
                              {
                                texto:
                                  t.estado === "padrao"
                                    ? "Criar a cópia e abrir no computador"
                                    : "Abrir no editor do computador",
                                aoEscolher: () => abrirNoComputador(t),
                              },
                              {
                                texto: "Voltar ao padrão do app",
                                desabilitado: t.estado === "padrao",
                                aoEscolher: () => setRestaurando(t),
                              },
                            ]}
                          />
                        </BarraAcoes>
                      }
                    >
                      <span className="config-item-texto">
                        <span className="config-item-nome">{t.rotulo}</span>
                        <span className="muted">{descricaoDe(t) ?? g.descricao}</span>
                      </span>
                    </ItemLista>
                  ))}
                </ListaItens>
              </Cartao>
            );
          })}
          {soEditados && !textos.some((t) => t.grupo !== "regrasDoProjeto" && t.estado === "editado") && (
            <EstadoVazio>Você ainda não editou nenhum texto: todos usam o padrão do app.</EstadoVazio>
          )}
          {aberto && (
            <p className="muted" role="status">
              Aberto: {aberto}
            </p>
          )}
          {falha && <Alerta tom="erro" titulo={falha} />}
        </>
      )}

      {restaurando && (
        <DialogoConfirmacao
          titulo={`Voltar ao padrão: ${restaurando.rotulo}`}
          aoCancelar={() => !ocupado && setRestaurando(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                disabled={ocupado}
                onClick={() => setRestaurando(null)}
              >
                Manter a minha versão
              </button>
              <button
                type="button"
                className="btn btn-danger"
                disabled={ocupado}
                onClick={() => restaurar(restaurando)}
              >
                Voltar ao padrão
              </button>
            </>
          }
        >
          <p>
            O app apaga a sua cópia em <code>{restaurando.arquivo}</code> e volta a usar o texto
            padrão. O que você escreveu se perde.
          </p>
        </DialogoConfirmacao>
      )}
    </Secao>
  );
}

/** O selo do estado do texto: editado em andamento, ilegível em atenção. */
function tomDoEstado(t: TextoEditavel): TomSelo {
  if (t.estado === "editado") return "andamento";
  if (t.estado === "ilegivel") return "atencao";
  return "neutro";
}
