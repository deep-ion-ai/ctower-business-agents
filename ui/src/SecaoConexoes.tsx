import { useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, EstadoVazio, Secao, Selo } from "./componentes";
import { useConexoes } from "./ConexoesContexto";
import { descreverFonte, descreverUsos, type ConexaoVista } from "./conexoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { FormConexao } from "./FormConexao";
import { PassosTeste } from "./PassosTeste";
import type { IntegracaoProntidao } from "./repositorios";
import { Icon, iconeDoNome } from "./shell/icons";

/**
 * Configurações › Conexões (#437): o GitHub, o GitLab, o Jira e os bancos cadastrados
 * uma vez e usados pelos projetos. Cada conexão mostra o tipo, o endereço, de onde vem
 * a credencial (nunca o valor), o último teste e os projetos que a usam. “Testar” roda
 * os passos de prontidão do descritor, cada um com a orientação; uma conexão em uso
 * não pode ser removida, e o app diz quais projetos a usam.
 */
export function SecaoConexoes() {
  const ctx = useConexoes();
  const [editando, setEditando] = useState<ConexaoVista | "nova" | null>(null);
  const [removendo, setRemovendo] = useState<ConexaoVista | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [testes, setTestes] = useState<Record<string, IntegracaoProntidao | "testando">>({});
  if (!ctx) return null;
  const { api, descritores, conexoes, carregando, recarregar } = ctx;

  const testar = (id: string) => {
    setTestes((t) => ({ ...t, [id]: "testando" }));
    api
      .testarConexao(id)
      .then((r) => setTestes((t) => ({ ...t, [id]: r })))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setTestes((t) => {
          const { [id]: _, ...resto } = t;
          return resto;
        });
      })
      .finally(() => void recarregar());
  };

  const remover = (c: ConexaoVista) => {
    setRemovendo(null);
    setFalha(null);
    api
      .removerConexao(c.conexao.id)
      .then(() => recarregar())
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  return (
    <Secao
      titulo="Conexões"
      acoes={
        <button
          type="button"
          className="btn btn-sm"
          disabled={descritores.length === 0}
          onClick={() => setEditando("nova")}
        >
          Nova conexão
        </button>
      }
    >
      <p className="muted">
        Cadastre cada sistema externo uma vez e escolha a conexão nos projetos. Trocar o token
        aqui vale para todos os projetos que usam a conexão, inclusive para os clones e a
        publicação. A credencial fica no chaveiro do sistema (ou no comando que a imprime),
        nunca no cadastro.
      </p>
      {ctx.falha && <Alerta tom="erro" titulo="Não deu para ler as conexões." >{ctx.falha}</Alerta>}
      {falha && <Alerta tom="erro" titulo={falha} />}
      {editando === "nova" && (
        <FormConexao
          api={api}
          descritores={descritores}
          aoCancelar={() => setEditando(null)}
          aoSalvar={() => {
            setEditando(null);
            void recarregar();
          }}
        />
      )}
      {!carregando && conexoes.length === 0 && editando !== "nova" && (
        <EstadoVazio>Nenhuma conexão cadastrada.</EstadoVazio>
      )}
      <ul className="lista-conexoes">
        {conexoes.map((c) => {
          const teste = testes[c.conexao.id];
          return (
            <li key={c.conexao.id} className="conexao" aria-label={c.conexao.nome}>
              <div className="cab-conexao">
                <Icon name={iconeDoNome(c.icone)} />
                <strong>{c.conexao.nome}</strong>
                <span className="muted">
                  {c.rotuloTipo} · <code>{c.conexao.endereco}</code>
                </span>
                {c.ultimoTeste && (
                  <Selo tom={c.ultimoTeste.pronta ? "concluido" : "erro"} titulo={c.ultimoTeste.resumo}>
                    {c.ultimoTeste.pronta ? "pronta" : "com falha"}
                  </Selo>
                )}
              </div>
              <p className="dica">
                Credencial: {descreverFonte(c.conexao.credencial, descritores, c.conexao.tipo)}
                {c.credencialGuardada === true && " (guardada)"}
                {c.credencialGuardada === false && " (nada guardado ainda)"}
              </p>
              <p className="dica">
                {c.usos.length > 0 ? `Usada por ${descreverUsos(c.usos)}` : "Nenhum projeto usa esta conexão."}
              </p>
              <div className="acoes">
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={teste === "testando"}
                  onClick={() => testar(c.conexao.id)}
                >
                  Testar
                </button>
                <button type="button" className="btn btn-sm" onClick={() => setEditando(c)}>
                  Editar
                </button>
                <button
                  type="button"
                  className="btn btn-sm"
                  aria-label={`Remover ${c.conexao.nome}`}
                  onClick={() => setRemovendo(c)}
                >
                  Remover
                </button>
              </div>
              <div role="status" aria-live="polite">
                {teste === "testando" && <span className="muted">Testando a conexão…</span>}
                {teste && teste !== "testando" && <PassosTeste resultado={teste} />}
              </div>
              {editando !== "nova" && editando?.conexao.id === c.conexao.id && (
                <FormConexao
                  api={api}
                  descritores={descritores}
                  inicial={c.conexao}
                  credencialGuardada={c.credencialGuardada === true}
                  aoCancelar={() => setEditando(null)}
                  aoSalvar={() => {
                    setEditando(null);
                    void recarregar();
                  }}
                />
              )}
            </li>
          );
        })}
      </ul>
      {removendo && (
        <DialogoConfirmacao
          titulo={`Remover a conexão ${removendo.conexao.nome}?`}
          aoCancelar={() => setRemovendo(null)}
          acoes={
            <>
              <button type="button" className="btn" autoFocus onClick={() => setRemovendo(null)}>
                Cancelar
              </button>
              <button type="button" className="btn btn-danger" onClick={() => remover(removendo)}>
                Remover
              </button>
            </>
          }
        >
          {removendo.usos.length > 0
            ? `Ela é usada por ${descreverUsos(removendo.usos)}.`
            : "Nenhum projeto usa esta conexão."}
        </DialogoConfirmacao>
      )}
    </Secao>
  );
}
