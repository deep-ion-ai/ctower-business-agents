import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Secao } from "./componentes";
import { useConexoes } from "./ConexoesContexto";
import { conexoesPara } from "./conexoes";
import {
  apiConsultasBancoTauri,
  type ApiConsultasBanco,
  type BancoDoProjeto,
} from "./consultasBanco";

/**
 * Bancos de dados do projeto (#440): as conexões de banco (Configurações › Conexões)
 * que as análises deste projeto podem usar. Cada análise ainda escolhe se usa uma
 * delas ("Usar o banco <nome> nesta análise"); sem isso, o agente nem vê a consulta ao
 * banco. Fora de um provedor de conexões, ou sem conexão de banco cadastrada, mostra
 * só a orientação.
 */
export function BancosDoProjeto({
  projeto,
  api = apiConsultasBancoTauri,
}: {
  projeto: string;
  api?: ApiConsultasBanco;
}) {
  const ctx = useConexoes();
  const [escolhidos, setEscolhidos] = useState<BancoDoProjeto[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [salvando, setSalvando] = useState(false);

  useEffect(() => {
    let ativo = true;
    Promise.resolve()
      .then(() => api.bancosProjeto(projeto))
      .then((b) => ativo && setEscolhidos(Array.isArray(b) ? b : []))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);

  const opcoes = ctx ? conexoesPara(ctx.conexoes, ctx.descritores, "banco") : [];
  const ids = new Set((escolhidos ?? []).map((b) => b.id));

  const alternar = (id: string, ligar: boolean) => {
    const novos = (escolhidos ?? []).map((b) => b.id).filter((x) => x !== id);
    if (ligar) novos.push(id);
    setSalvando(true);
    setFalha(null);
    api
      .salvarBancosProjeto(projeto, novos)
      .then(setEscolhidos)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  return (
    <Secao titulo="Bancos de dados" className="bancos-projeto">
      <p className="muted">
        Conexões de banco só de leitura que as análises deste projeto podem usar. Em cada
        análise, você escolhe se o agente pode consultar o banco; cada consulta espera a sua
        aprovação, e os dados pessoais chegam mascarados.
      </p>
      {falha && <Alerta tom="erro" titulo={falha} />}
      {escolhidos === null && !falha && <p className="muted">Carregando…</p>}
      {escolhidos !== null && opcoes.length === 0 && (
        <p className="muted">
          Nenhuma conexão de banco cadastrada. Cadastre uma em Configurações › Conexões.
        </p>
      )}
      {escolhidos !== null && opcoes.length > 0 && (
        <fieldset disabled={salvando} className="lista-bancos" aria-label="Bancos do projeto">
          {opcoes.map((c) => (
            <label key={c.conexao.id} className="check">
              <input
                type="checkbox"
                checked={ids.has(c.conexao.id)}
                onChange={(e) => alternar(c.conexao.id, e.target.checked)}
              />{" "}
              {c.conexao.nome} <span className="muted">({c.rotuloTipo})</span>
            </label>
          ))}
        </fieldset>
      )}
      {escolhidos
        ?.filter((b) => !b.cadastrada)
        .map((b) => (
          <p key={b.id} className="attention-text">
            A conexão “{b.id}” não está mais cadastrada.{" "}
            <button className="btn btn-sm" onClick={() => alternar(b.id, false)}>
              Tirar do projeto
            </button>
          </p>
        ))}
    </Secao>
  );
}
